use axum::{Router, extract::Query, routing::get};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    authentication::AuthUser,
    db::{self},
    extract,
    htmf_response::HtmfResponse,
    response_error::ResponseResult,
    server::AppState,
    views,
    views::layout,
};
pub fn router() -> Router<AppState> {
    let router: Router<AppState> = Router::new();
    router.route("/search", get(get_search))
}

#[derive(Deserialize, Serialize)]
pub struct SearchQuery {
    /// The words to search for
    pub q: String,
    pub after_bookmark_id: Option<Uuid>,
}

async fn get_search(
    auth_user: AuthUser,
    extract::Tx(mut tx): extract::Tx,

    Query(query): Query<SearchQuery>,
) -> ResponseResult<HtmfResponse> {
    let results = db::search::search(
        &mut tx,
        &query.q,
        auth_user.ap_user_id,
        query.after_bookmark_id,
    )
    .await?;
    let mut layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;
    layout.previous_search_input = Some(query.q);
    Ok(HtmfResponse(views::search_results::view(
        &views::search_results::Data { layout, results },
    )))
}
