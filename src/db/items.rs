//! todo: better name than "items"
//! maybe "link destinations"?

use anyhow::Context;
use sqlx::query;
use uuid::Uuid;

use super::{AppTx, LinkDestination};
use crate::response_error::ResponseResult;

#[expect(dead_code, reason = "We'll use this for global search later")]
pub async fn search(
    tx: &mut AppTx,
    term: &str,
    ap_user_id: Uuid,
) -> ResponseResult<Vec<LinkDestination>> {
    let jsons = query!(
        r#"
            select to_jsonb(bookmarks.*) as item
            from bookmarks
            where bookmarks.title ilike '%' || $1 || '%'
            and bookmarks.ap_user_id = $2
            union
            select to_jsonb(lists.*) as item
            from lists
            where lists.title ilike '%' || $1 || '%'
            and lists.ap_user_id = $2
            limit 10
        "#,
        term,
        ap_user_id
    )
    .fetch_all(&mut **tx)
    .await?;

    let results = jsons
        .into_iter()
        .map(|row| Ok(serde_json::from_value(row.item.into())?))
        .collect::<anyhow::Result<Vec<LinkDestination>>>()?;

    Ok(results)
}

pub async fn by_id(tx: &mut AppTx, id: Uuid) -> ResponseResult<LinkDestination> {
    let json = query!(
        r#"
            select to_jsonb(bookmarks.*) as item
            from bookmarks
            where bookmarks.id = $1
            union
            select to_jsonb(lists.*) as item
            from lists
            where lists.id = $1
        "#,
        id
    )
    .fetch_one(&mut **tx)
    .await?;

    let results =
        serde_json::from_value(json.item.into()).context("Failed to deserialize item from DB")?;

    Ok(results)
}
