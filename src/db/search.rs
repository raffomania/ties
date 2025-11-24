use sqlx::{query, query_as};
use uuid::Uuid;

use super::AppTx;
use crate::response_error::ResponseResult;

pub enum PreviousPage {
    // We already are on the first page.
    DoesNotExist,
    /// The previous page is the first page, so we have no bookmark id to query
    /// "after", but still need to show the link.
    IsFirstPage,
    /// There's another page before the previous page, so we can reference the
    /// last bookmark of that page.
    AfterBookmarkId(Uuid),
}

pub struct Results {
    pub bookmarks: Vec<Result>,
    pub previous_page: PreviousPage,
    pub next_page_after_bookmark_id: Option<Uuid>,
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
    after_bookmark_id: Option<Uuid>,
) -> ResponseResult<Results> {
    let mut bookmarks = query_as!(
        Result,
        r#"
            select title, url as bookmark_url, id as bookmark_id
            from bookmarks
            where (bookmarks.title ilike '%' || $1 || '%')
                and bookmarks.ap_user_id = $2
                and ($3::uuid is null or bookmarks.id > $3)
            order by bookmarks.id asc
            limit 51
        "#,
        term,
        ap_user_id,
        after_bookmark_id
    )
    .fetch_all(&mut **tx)
    .await?;

    let total_count = query!(
        r#"
            select count(bookmarks.id) as "count!" from bookmarks
            where (bookmarks.title ilike '%' || $1 || '%')
                and bookmarks.ap_user_id = $2
        "#,
        term,
        ap_user_id
    )
    .fetch_one(&mut **tx)
    .await?
    .count;

    let next_page_exists = bookmarks.len() == 51;
    if next_page_exists {
        bookmarks.pop();
    }
    let next_page_after_bookmark_id = next_page_exists
        .then_some(bookmarks.last().map(|b| b.bookmark_id))
        .flatten();

    let first_id = bookmarks.first().map(|b| b.bookmark_id);
    // Check if there are *any* bookmarks before the first of the current page.
    // If so, fetch the ids for the previous page and take the first one.
    // We need to fetch multiple bookmarks because we don't know how small the
    // previous page is.
    let previous_bookmarks = query!(
        r#"
            select bookmarks.id
            from bookmarks
            where (bookmarks.title ilike '%' || $1 || '%')
                and bookmarks.ap_user_id = $2
                and ($3::uuid is null or bookmarks.id < $3)
            order by bookmarks.id desc
            limit 51
        "#,
        term,
        ap_user_id,
        first_id
    )
    .fetch_all(&mut **tx)
    .await?;
    let previous_page = if let Some(last) = previous_bookmarks.last() {
        if previous_bookmarks.len() == 51 {
            // There's another page before the previous page, so we can reference the last
            // bookmark of that page.
            PreviousPage::AfterBookmarkId(last.id)
        } else {
            PreviousPage::IsFirstPage
        }
    } else {
        PreviousPage::DoesNotExist
    };

    Ok(Results {
        bookmarks,
        previous_page,
        next_page_after_bookmark_id,
        total_count,
    })
}
