use sqlx::prelude::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{db::AppTx, response_error::ResponseResult};

#[derive(FromRow)]
pub struct Activity {
    pub bookmark_id: Uuid,
    pub created_at: OffsetDateTime,

    pub url: String,
    pub title: Option<String>,
    pub ap_id: String,

    pub username: String,
}

pub async fn public_activity_in_list(
    tx: &mut AppTx,
    list_id: Uuid,
) -> ResponseResult<Vec<Activity>> {
    // TODO: better detection for which users are linked to the list
    let news = sqlx::query_as!(
        Activity,
        r#"
        with followed_users as materialized (
            select distinct ap_users.id as ap_user_id, ap_users.username
            from ap_users
            inner join bookmarks on bookmarks.url like '%' || ap_users.username || '%'
            inner join links on links.dest_bookmark_id = bookmarks.id
                and links.src_list_id = $1
        )
        select distinct on (bookmarks.id)
            bookmarks.id as "bookmark_id", bookmarks.created_at,
            bookmarks.url, bookmarks.title, bookmarks.ap_id,
            followed_users.username
        from bookmarks
        inner join links on links.dest_bookmark_id = bookmarks.id
        inner join followed_users on bookmarks.ap_user_id = followed_users.ap_user_id
        inner join lists on links.src_list_id = lists.id
            and not lists.private
        order by bookmarks.id, bookmarks.created_at
        limit 5
        "#,
        list_id
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(news)
}
