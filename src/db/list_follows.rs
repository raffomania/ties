use sqlx::{FromRow, query, query_as};
use uuid::Uuid;

use super::AppTx;
use crate::response_error::ResponseResult;

#[allow(clippy::struct_field_names)]
pub struct Insert {
    pub list_id: Uuid,
    pub bookmark_id: Uuid,
    pub followed_ap_user_id: Uuid,
    pub follow_id: Uuid,
}

pub async fn insert(tx: &mut AppTx, insert: Insert) -> ResponseResult<()> {
    query!(
        r"
        insert into list_follows
        (
            list_id,
            bookmark_id,
            followed_ap_user_id,
            follow_id
        )
        values ($1, $2, $3, $4)
        ",
        insert.list_id,
        insert.bookmark_id,
        insert.followed_ap_user_id,
        insert.follow_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

#[expect(dead_code, reason = "Kept for reference on the DB schema")]
#[derive(FromRow, Debug)]
pub struct ListFollow {
    pub id: Uuid,
}

pub async fn read(
    tx: &mut AppTx,
    list_id: Uuid,
    bookmark_id: Uuid,
    follow_id: Uuid,
) -> ResponseResult<ListFollow> {
    let row = query_as!(
        ListFollow,
        r"
        select id from list_follows
        where list_id = $1 and bookmark_id = $2 and follow_id = $3
        ",
        list_id,
        bookmark_id,
        follow_id,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(row)
}

pub async fn remove_by_list_id_if_exists(tx: &mut AppTx, list_id: Uuid) -> ResponseResult<()> {
    query!(
        r"
        delete from list_follows
        where list_id = $1
        ",
        list_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}
