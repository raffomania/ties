use anyhow::{Context, anyhow};
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderValue, header::SET_COOKIE},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use garde::{Report, Validate};
use serde::Deserialize;
use serde_qs::web::{QsForm, QsQuery};

use crate::{
    authentication::{self, AuthUser},
    db,
    db::sessions::Session,
    extract::{self},
    forms::users::{CreateOidcUser, Login, OidcLoginQuery, OidcSelectUsername},
    htmf_response::HtmfResponse,
    oidc::{self},
    response_error::{ResponseError, ResponseResult},
    server::AppState,
    session,
    views::{self, layout, login, oidc_select_username},
};

/// Attach a `Set-Cookie` header (if present) to `res`.
fn attach_cookie(res: &mut Response, cookie: HeaderValue) {
    res.headers_mut().insert(SET_COOKIE, cookie);
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", get(get_login).post(post_login))
        .route("/login_oidc_redirect", get(get_login_oidc_redirect))
        .route("/login_oidc_redirect", post(post_login_oidc_redirect))
        .route("/login_oidc", get(get_login_oidc))
        .route("/login_demo", post(post_login_demo))
        .route("/logout", post(logout))
        .route("/user/{username}", get(get_profile))
}

#[axum::debug_handler]
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

    let logged_in =
        authentication::login(&mut tx, session, &input.credentials, &state.base_url).await;
    let cookie = match logged_in {
        Ok(cookie) => cookie,
        Err(e) => {
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
    };
    tx.commit().await?;
    let redirect_to = input.previous_uri.unwrap_or(state.base_url);
    let mut res = Redirect::to(redirect_to.as_str()).into_response();
    attach_cookie(&mut res, cookie);
    Ok(res)
}

async fn get_login_oidc(
    State(state): State<AppState>,
    mut session: Session,
    extract::Tx(mut tx): extract::Tx,
) -> ResponseResult<Response> {
    // TODO: Store the CSRF and none states in a way that is more secure than this,
    // although the current method is already quite secure.
    let oidc_config = state
        .oidc_state
        .get_config()
        .context("OIDC client not configured")?;

    let attempt = oidc::LoginAttempt::new(&oidc_config.client);
    let authorize_url = attempt.authorize_url.clone();

    session.contents.oidc_login_attempt = Some(attempt);
    let cookie = session.persist(&mut tx, &state.base_url).await?;
    tx.commit().await?;

    let mut res = Redirect::to(authorize_url.as_str()).into_response();
    attach_cookie(&mut res, cookie);
    Ok(res)
}

async fn get_login_oidc_redirect(
    extract::Tx(mut tx): extract::Tx,
    session: Session,
    QsQuery(query): QsQuery<OidcLoginQuery>,
    state: State<AppState>,
) -> ResponseResult<Response> {
    let oidc_config = state
        .oidc_state
        .clone()
        .get_config()
        .context("OIDC not configured")?;

    let oidc_session = session
        .contents
        .oidc_login_attempt
        .clone()
        .context("oidc login attempt not found in session")?;

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
            let cookie = session
                .persist_logged_in_user(&mut tx, &existing_user, &state.base_url)
                .await?;
            tx.commit().await?;

            let mut res = Redirect::to("/").into_response();
            attach_cookie(&mut res, cookie);

            Ok(res)
        }
        // Show new users a form to choose a username
        Err(ResponseError::NotFound) => {
            let mut session = session.rotate(&mut tx).await?;
            session.contents.oidc_user_info = Some(authed_oidc_info);
            let cookie = session.persist(&mut tx, &state.base_url).await?;
            tx.commit().await?;

            let mut res = HtmfResponse(oidc_select_username::view(
                views::oidc_select_username::Data::default(),
            ))
            .into_response();

            attach_cookie(&mut res, cookie);

            Ok(res)
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

    let authed_oidc_info = session
        .contents
        .oidc_user_info
        .as_ref()
        .context("oidc data not found in session")?;

    let create_oidc_user = CreateOidcUser {
        oidc_id: authed_oidc_info.oidc_id.clone(),
        email: authed_oidc_info.email.clone(),
        username: input.username,
    };

    if let Err(e) = create_oidc_user.validate() {
        return Err(anyhow!("Invalid OIDC user data received").context(e).into());
    }

    let cookie = authentication::create_and_login_oidc_user(
        &mut tx,
        session,
        create_oidc_user,
        &state.base_url,
    )
    .await?;

    tx.commit().await?;

    let mut res = Redirect::to("/").into_response();
    attach_cookie(&mut res, cookie);
    Ok(res)
}

async fn post_login_demo(
    extract::Tx(mut tx): extract::Tx,
    session: Session,
    State(state): State<AppState>,
) -> ResponseResult<Response> {
    let cookie =
        authentication::create_and_login_temp_user(&mut tx, session, &state.base_url).await?;
    tx.commit().await?;

    let mut res = Redirect::to("/").into_response();
    attach_cookie(&mut res, cookie);
    Ok(res)
}

#[derive(Deserialize)]
struct LoginQuery {
    previous_uri: Option<String>,
}

// TODO: redirect to homepage if already logged in
// https://github.com/raffomania/ties/issues/177
async fn get_login(
    QsQuery(query): QsQuery<LoginQuery>,
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

// TODO: set this route as @url in activitypub person objects
// https://www.w3.org/TR/activitystreams-vocabulary/#dfn-url
// https://github.com/raffomania/ties/issues/150
async fn get_profile(
    extract::Tx(mut tx): extract::Tx,
    Path(handle): Path<String>,
    State(state): State<AppState>,
    auth_user: Option<AuthUser>,
) -> ResponseResult<HtmfResponse> {
    let layout = layout::Template::from_db(&mut tx, auth_user.as_ref()).await?;

    let ap_user = db::ap_users::read_by_username(
        &mut tx,
        crate::federation::webfinger::Resource::parse_handle(&handle, &state.base_url)?,
    )
    .await?;
    let maybe_local_user = db::users::local_by_ap_user_id(&mut tx, ap_user.id).await?;
    let public_lists = if let Some(user) = &maybe_local_user {
        db::lists::list_public_by_user(&mut tx, user.ap_user_id).await?
    } else {
        Vec::new()
    };

    let invited_by_user = if let Some(user) = &maybe_local_user
        && let Some(invited_by_id) = user.invited_by
    {
        Some(db::ap_users::read_by_user_id(&mut tx, invited_by_id).await?)
    } else {
        None
    };

    let elem = views::profile::view(
        tx,
        &views::profile::Data {
            layout,
            ap_user,
            public_lists,
            invited_by_user,
        },
    )
    .await?;

    Ok(HtmfResponse(elem))
}

async fn logout(
    extract::Tx(mut tx): extract::Tx,
    State(state): State<AppState>,
    session: Session,
) -> ResponseResult<Response> {
    db::sessions::delete(&mut tx, session).await?;
    tx.commit().await?;

    let mut res = Redirect::to("/login").into_response();
    attach_cookie(&mut res, session::clear_cookie_header(&state.base_url)?);
    Ok(res)
}
