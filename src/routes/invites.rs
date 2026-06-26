use axum::{Router, extract::State, routing::post};

use crate::{
    authentication::AuthUser,
    db, extract,
    htmf_response::HtmfResponse,
    response_error::ResponseResult,
    server::AppState,
    views::{self, layout},
};

pub fn router() -> Router<AppState> {
    Router::new().route("/invites/create", post(post_create))
}

async fn post_create(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    State(state): State<AppState>,
) -> ResponseResult<HtmfResponse> {
    // A bit lazy, but this makes sure old invites don't pile up indefinitely
    db::invites::delete_expired(&mut tx).await?;

    let invite = db::invites::insert(&mut tx, auth_user.user_id).await?;
    Ok(views::create_invite::view(views::create_invite::Data {
        layout: layout::Template::from_db(&mut tx, Some(&auth_user)).await?,
        token: invite.token,
        base_url: state.base_url,
    })?
    .into())
}
