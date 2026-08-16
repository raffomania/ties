use axum::{
    Router,
    extract::{Path, State},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use garde::Validate;
use reqwest::header::SET_COOKIE;
use serde_qs::web::QsForm;

use crate::{
    authentication::{self, AuthUser},
    db::{self, sessions::Session},
    extract, forms,
    htmf_response::HtmfResponse,
    response_error::ResponseResult,
    server::AppState,
    views::{self, layout},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/invites/create", post(post_create).get(get_create))
        .route("/invites/{token}", get(get_accept).post(post_accept))
}

async fn post_create(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> ResponseResult<HtmfResponse> {
    // A bit lazy, but this makes sure old invites don't pile up indefinitely
    db::invites::delete_expired(&mut tx).await?;

    let invite = db::invites::insert(&mut tx, auth_user.user_id).await?;
    let layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;
    tx.commit().await?;

    Ok(
        views::create_invite::created(views::create_invite::Created {
            layout,
            token: invite.token,
            base_url: state.base_url,
        })?
        .into(),
    )
}

/// Redirect GET requests to the index page since that's where the button for
/// creating a new invite is.
/// This should only happen if users reload the posted invite creation page.
async fn get_create(_auth_user: AuthUser) -> Response {
    Redirect::to("/").into_response()
}

async fn get_accept(
    extract::Tx(mut tx): extract::Tx,
    Path(token): Path<String>,
) -> ResponseResult<HtmfResponse> {
    let data = views::accept_invite::Data::from_db(&mut tx, &token).await?;

    Ok(views::accept_invite::view(&data).into())
}

async fn post_accept(
    extract::Tx(mut tx): extract::Tx,
    Path(token): Path<String>,
    State(state): State<AppState>,
    session: Session,
    QsForm(form_input): QsForm<forms::users::CreateUser>,
) -> ResponseResult<Response> {
    let mut data = views::accept_invite::Data::from_db(&mut tx, &token).await?;

    if let Err(errors) = form_input.validate() {
        data.errors = errors.into();
    }

    if db::users::by_username(&mut tx, &form_input.username)
        .await
        .is_ok()
    {
        data.errors.0.append(
            garde::Path::new("username"),
            garde::Error::new("This username is already taken"),
        );
    }

    if !data.errors.0.is_empty() {
        data.form_input = Some(form_input.clone());
        return Ok(HtmfResponse(views::accept_invite::view(&data)).into_response());
    }

    db::invites::delete(&mut tx, &token).await?;
    db::users::insert(
        &mut tx,
        form_input.clone(),
        Some(data.invite.invited_by),
        &state.base_url,
    )
    .await?;

    let cookie = authentication::login(
        &mut tx,
        session,
        &forms::users::Credentials {
            username: form_input.username,
            password: form_input.password,
        },
        &state.base_url,
    )
    .await?;

    tx.commit().await?;

    let mut res = Redirect::to("/").into_response();

    res.headers_mut().insert(SET_COOKIE, cookie);

    Ok(res)
}
