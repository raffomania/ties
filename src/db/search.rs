use sqlx::{query, query_as};
use uuid::Uuid;

use super::AppTx;
use crate::response_error::ResponseResult;

const PAGE_SIZE: i64 = 50;

pub struct Results {
    pub bookmarks: Vec<Result>,
    pub previous_page: Option<i64>,
    pub next_page: Option<i64>,
    pub total_count: i64,
}

pub struct Result {
    pub title: String,
    pub bookmark_id: Uuid,
    pub bookmark_url: String,
}

pub async fn search(
    tx: &mut AppTx,
    term: &str,
    ap_user_id: Uuid,
    page: i64,
) -> ResponseResult<Results> {
    // Note: when changing the filtering here, remember to update it in the second
    // query below as well.
    let bookmarks = query_as!(
        Result,
        r#"
            select title, url as bookmark_url, id as bookmark_id
            from bookmarks
            where bookmarks.search @@ plainto_tsquery($1)
                and bookmarks.ap_user_id = $2
            order by ts_rank('{0.0, 1.0, 0.4, 1.0}'::float4[], bookmarks.search, plainto_tsquery($1)) desc
            limit $3
            offset $4
        "#,
        term,
        ap_user_id,
        PAGE_SIZE,
        page * PAGE_SIZE
    )
    .fetch_all(&mut **tx)
    .await?;

    let total_count = query!(
        r#"
            select count(bookmarks.id) as "count!" from bookmarks
            where bookmarks.search @@ plainto_tsquery($1)
                and bookmarks.ap_user_id = $2
        "#,
        term,
        ap_user_id
    )
    .fetch_one(&mut **tx)
    .await?
    .count;

    let previous_page = (page > 0).then_some(page - 1);

    let next_page_exists = total_count > (page + 1) * PAGE_SIZE;
    let next_page = next_page_exists.then_some(page + 1);

    Ok(Results {
        bookmarks,
        previous_page,
        next_page,
        total_count,
    })
}
