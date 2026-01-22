use axum::{Router, extract::State, routing::get};

use crate::{
    authentication::AuthUser,
    db, extract,
    htmf_response::HtmfResponse,
    response_error::ResponseResult,
    server::AppState,
    views::{self, layout},
};

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(index))
}

async fn index(
    auth_user: AuthUser,
    extract::Tx(mut tx): extract::Tx,
    State(state): State<AppState>,
) -> ResponseResult<HtmfResponse> {
    let layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;
    let authed_info = db::layout::by_ap_user_id(&mut tx, auth_user.ap_user_id).await?;

    Ok(views::index::view(&views::index::Data {
        layout: &layout,
        base_url: &state.base_url,
        authed_info: &authed_info,
    })
    .into())
}
