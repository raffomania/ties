use axum::{Router, extract::State, routing::get};

use crate::{
    authentication::AuthUser,
    extract,
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
    Ok(views::index::view(&views::index::Data {
        layout: &layout::Template::from_db(&mut tx, Some(&auth_user)).await?,
        base_url: &state.base_url,
    })
    .into())
}
