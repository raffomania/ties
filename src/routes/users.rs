use anyhow::{Context, anyhow};
use axum::{
    Router,
    extract::{Path, Query, State},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use garde::{Report, Validate};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    authentication::{self, AuthUser},
    db,
    extract::{self, qs_form::QsForm},
    forms::users::{CreateOidcUser, Login, OidcLoginQuery, OidcSelectUsername},
    htmf_response::HtmfResponse,
    oidc::{self},
    response_error::{ResponseError, ResponseResult},
    server::AppState,
    views::{self, layout, login, oidc_select_username, users::ProfileTemplate},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", get(get_login).post(post_login))
        .route("/login_oidc_redirect", get(get_login_oidc_redirect))
        .route("/login_oidc_redirect", post(post_login_oidc_redirect))
        .route("/login_oidc", get(get_login_oidc))
        .route("/login_demo", post(post_login_demo))
        .route("/logout", post(logout))
        .route("/start", get(get_start_page))
        .route("/user/{username}", get(get_profile))
}

async fn post_login(
    extract::Tx(mut tx): extract::Tx,
    session: Session,
    State(state): State<AppState>,
    QsForm(input): QsForm<Login>,
) -> ResponseResult<Response> {
    if let Err(errors) = input.validate() {
        return Ok(HtmfResponse(login::login(&login::Template::new(
            errors,
            input,
            state.oidc_state,
        )))
        .into_response());
    }

    let logged_in = authentication::login(&mut tx, session, &input.credentials).await;
    if let Err(e) = logged_in {
        tracing::debug!("{e:?}");
        let mut errors = Report::new();
        errors.append(
            garde::Path::new("root"),
            garde::Error::new("Username or password not correct"),
        );
        return Ok(HtmfResponse(login::login(&login::Template::new(
            errors,
            input,
            state.oidc_state,
        )))
        .into_response());
    }

    let redirect_to = input.previous_uri.unwrap_or(state.base_url);

    Ok(Redirect::to(redirect_to.as_str()).into_response())
}

async fn get_login_oidc(
    State(state): State<AppState>,
    session: Session,
) -> ResponseResult<Response> {
    // TODO: Store the CSRF and none states in a way that is more secure than this,
    // although the current method is already quite secure.
    let oidc_config = state
        .oidc_state
        .get_config()
        .context("OIDC client not configured")?;
    let attempt = oidc::LoginAttempt::new(&oidc_config.client);
    let authorize_url = attempt.authorize_url.clone();
    attempt.save_in_session(&session).await?;

    Ok(Redirect::to(authorize_url.as_str()).into_response())
}

async fn get_login_oidc_redirect(
    session: Session,
    Query(query): Query<OidcLoginQuery>,
    state: State<AppState>,
    extract::Tx(mut tx): extract::Tx,
) -> ResponseResult<Response> {
    let oidc_config = state
        .oidc_state
        .clone()
        .get_config()
        .context("OIDC not configured")?;

    let oidc_session: oidc::LoginAttempt = oidc::LoginAttempt::from_session(&session).await?;
    let authed_oidc_info = oidc_session
        .login(
            &oidc_config.client,
            &oidc_config.reqwest_client,
            query.state,
            query.code,
        )
        .await?;

    let existing_user = db::users::by_oidc_id(&mut tx, &authed_oidc_info.oidc_id).await;
    match existing_user {
        // Authenticate existing users in session
        Ok(existing_user) => {
            authentication::login_oidc_user(&session, &existing_user).await?;
            Ok(Redirect::to("/").into_response())
        }
        // Show new users a form to choose a username
        Err(ResponseError::NotFound) => {
            authed_oidc_info.save_in_session(&session).await?;
            Ok(HtmfResponse(oidc_select_username::view(
                views::oidc_select_username::Data::default(),
            ))
            .into_response())
        }
        Err(e) => Err(e),
    }
}

async fn post_login_oidc_redirect(
    session: Session,
    extract::Tx(mut tx): extract::Tx,
    State(state): State<AppState>,
    QsForm(input): QsForm<OidcSelectUsername>,
) -> ResponseResult<Response> {
    if let Err(errors) = input.validate() {
        return Ok(HtmfResponse(views::oidc_select_username::view(
            views::oidc_select_username::Data {
                errors: errors.into(),
                form_input: input,
            },
        ))
        .into_response());
    }

    let authed_oidc_info = oidc::AuthenticatedOidcUserInfo::from_session(&session).await?;

    let create_oidc_user = CreateOidcUser {
        oidc_id: authed_oidc_info.oidc_id,
        email: authed_oidc_info.email,
        username: input.username,
    };

    if let Err(e) = create_oidc_user.validate() {
        return Err(anyhow!("Invalid OIDC user data received").context(e).into());
    }

    authentication::create_and_login_oidc_user(
        &mut tx,
        &session,
        create_oidc_user,
        &state.base_url,
    )
    .await?;

    tx.commit().await?;

    Ok(Redirect::to("/").into_response())
}

async fn post_login_demo(
    extract::Tx(mut tx): extract::Tx,
    session: Session,
    State(state): State<AppState>,
) -> ResponseResult<Response> {
    authentication::create_and_login_temp_user(&mut tx, session, &state.base_url).await?;
    tx.commit().await?;

    Ok(Redirect::to("/").into_response())
}

#[derive(Deserialize)]
struct LoginQuery {
    previous_uri: Option<String>,
}

// TODO: redirect to homepage if already logged in
// https://github.com/raffomania/ties/issues/177
async fn get_login(
    Query(query): Query<LoginQuery>,
    State(state): State<AppState>,
) -> ResponseResult<Response> {
    if state.demo_mode {
        Ok(HtmfResponse(views::login_demo::view()).into_response())
    } else {
        let previous_uri = query
            .previous_uri
            .map(|u| state.base_url.join(&u))
            .transpose()?;

        Ok(HtmfResponse(login::login(&login::Template::new(
            Report::new(),
            Login {
                previous_uri,
                ..Default::default()
            },
            state.oidc_state,
        )))
        .into_response())
    }
}

async fn get_start_page(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> ResponseResult<HtmfResponse> {
    let layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;

    Ok(HtmfResponse(views::users::start_page(&ProfileTemplate {
        layout,
        base_url: state.base_url,
    })))
}

// TODO: set this route as @url in activitypub person objects
// https://www.w3.org/TR/activitystreams-vocabulary/#dfn-url
// https://github.com/raffomania/ties/issues/150
async fn get_profile(
    extract::Tx(mut tx): extract::Tx,
    Path(handle): Path<String>,
    State(state): State<AppState>,
) -> ResponseResult<HtmfResponse> {
    let layout = layout::Template::from_db(&mut tx, None).await?;

    let ap_user = db::ap_users::read_by_username(
        &mut tx,
        crate::federation::webfinger::Resource::parse_handle(&handle, &state.base_url)?,
    )
    .await?;
    let maybe_user = db::users::by_ap_user_id(&mut tx, ap_user.id).await?;
    let public_lists = if let Some(user) = maybe_user {
        db::lists::list_public_by_user(&mut tx, user.ap_user_id).await?
    } else {
        Vec::new()
    };

    let elem = views::profile::view(
        tx,
        &views::profile::Data {
            layout,
            ap_user,
            public_lists,
        },
    )
    .await?;

    Ok(HtmfResponse(elem))
}

async fn logout(auth_user: AuthUser) -> ResponseResult<Redirect> {
    auth_user.logout().await?;
    Ok(Redirect::to("/login"))
}
