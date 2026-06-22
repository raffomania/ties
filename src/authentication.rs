use anyhow::{Context, anyhow};
use argon2::PasswordVerifier;
use axum::{
    extract::{FromRequestParts, OptionalFromRequestParts, OriginalUri},
    http::{HeaderValue, request::Parts},
    response::Redirect,
};
use garde::Validate;
use percent_encoding::utf8_percent_encode;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::{
    db::{self, AppTx, sessions::Session},
    forms::users::{CreateOidcUser, CreateUser, Credentials},
    response_error::{ResponseError, ResponseResult},
    server::AppState,
};

pub fn hash_password(password: &str) -> ResponseResult<String> {
    let salt =
        argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);

    #[cfg(test)]
    let argon2 = argon2::Argon2::new(
        argon2::Algorithm::default(),
        argon2::Version::default(),
        #[expect(clippy::unwrap_in_result, reason = "test code")]
        argon2::Params::new(
            // Speed up tests by using the minimum cost params.
            argon2::Params::MIN_M_COST,
            argon2::Params::MIN_T_COST,
            argon2::Params::MIN_P_COST,
            None,
        )
        .unwrap(),
    );

    #[cfg(not(test))]
    let argon2 = argon2::Argon2::default();

    Ok(
        argon2::PasswordHasher::hash_password(&argon2, password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Failed to hash password: {e}"))?
            .to_string(),
    )
}

pub fn verify_password(user: &db::User, password: &str) -> ResponseResult<()> {
    let existing_hash = user
        .password_hash
        .as_ref()
        .context("User has no password set")?;
    let password_hash = &argon2::PasswordHash::new(existing_hash)
        .map_err(|e| anyhow!("Failed to create password hash: {e}"))?;

    argon2::Argon2::default()
        .verify_password(password.as_bytes(), password_hash)
        .map_err(|_e| ResponseError::NotAuthenticated)?;

    Ok(())
}

pub async fn login(
    tx: &mut AppTx,
    session: Session,
    creds: &Credentials,
) -> ResponseResult<HeaderValue> {
    let user = db::users::by_username(tx, &creds.username).await?;

    verify_password(&user, &creds.password)?;

    session.persist_logged_in_user(tx, &user).await
}

pub async fn create_and_login_temp_user(
    tx: &mut AppTx,
    session: Session,
    base_url: &Url,
) -> ResponseResult<HeaderValue> {
    let username =
        friendly_zoo::Zoo::new(friendly_zoo::Species::CustomDelimiter('_'), 1).generate();
    let password = Uuid::new_v4().to_string();
    let create = CreateUser { username, password };
    create.validate().context("Invalid demo user generated")?;
    let user = db::users::insert(tx, create, base_url).await?;

    session.persist_logged_in_user(tx, &user).await
}

pub async fn create_and_login_oidc_user(
    tx: &mut AppTx,
    session: Session,
    create_oidc_user: CreateOidcUser,
    base_url: &Url,
) -> ResponseResult<HeaderValue> {
    let user = db::users::by_oidc_id(tx, &create_oidc_user.oidc_id).await;

    let user = match user {
        Ok(user) => user,
        Err(ResponseError::NotFound) => {
            db::users::insert_oidc(tx, create_oidc_user, base_url).await?
        }
        Err(_) => return Err(anyhow!("Failed to look up user by OIDC id").into()),
    };

    session.persist_logged_in_user(tx, &user).await
}

/// Extractor for requiring a logged in user in routes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub ap_user_id: Uuid,
}

impl AuthUser {
    pub fn from_session(session: &Session) -> ResponseResult<Self> {
        let value = session
            .contents
            .auth_user
            .as_ref()
            .ok_or(ResponseError::NotAuthenticated)?;

        Ok(Self {
            user_id: value.user_id,
            ap_user_id: value.ap_user_id,
        })
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = Redirect;

    async fn from_request_parts(
        req: &mut Parts,
        state: &AppState,
    ) -> std::result::Result<Self, Self::Rejection> {
        let uri = OriginalUri::from_request_parts(req, state).await.unwrap();

        let redirect_after_login = uri
            .path_and_query()
            .map(axum::http::uri::PathAndQuery::as_str)
            .unwrap_or_default();
        let redirect_after_login =
            utf8_percent_encode(redirect_after_login, percent_encoding::NON_ALPHANUMERIC)
                .to_string();

        let redirect_to = format!("/login?previous_uri={redirect_after_login}");
        let error_redirect = Redirect::to(&redirect_to);

        let session = Session::from_request_parts(req, state).await.map_err(|e| {
            tracing::error!("{e:?}");
            error_redirect.clone()
        })?;

        let auth_user = AuthUser::from_session(&session);
        if let Err(ResponseError::NotAuthenticated) = auth_user {
            return Err(error_redirect);
        }

        auth_user.map_err(|e| {
            tracing::error!("{e:?}");
            error_redirect
        })
    }
}

impl OptionalFromRequestParts<AppState> for AuthUser {
    type Rejection = ResponseError;

    async fn from_request_parts(
        req: &mut Parts,
        state: &AppState,
    ) -> std::result::Result<Option<Self>, Self::Rejection> {
        let session = Session::from_request_parts(req, state).await?;

        let auth_user = AuthUser::from_session(&session);
        if let Err(ResponseError::NotAuthenticated) = auth_user {
            return Ok(None);
        }

        Ok(Some(auth_user?))
    }
}
