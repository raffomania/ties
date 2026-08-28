use sqlx::{prelude::FromRow, query, query_as};
use uuid::Uuid;

use crate::{db::AppTx, response_error::ResponseResult};

#[expect(dead_code, reason = "Kept for reference on the DB schema")]
#[derive(FromRow, Debug)]
pub struct Follow {
    pub id: Uuid,

    /// The AP user that is following
    pub follower_id: Uuid,
    /// The AP user being followed
    pub following_id: Uuid,
}

pub struct Insert {
    pub follower_ap_user_id: Uuid,
    pub following_ap_user_id: Uuid,
}

pub async fn upsert(tx: &mut AppTx, insert: Insert) -> ResponseResult<Follow> {
    // Don't return the follow because the `on conflict ... do nothing` won't return
    // anything on conflict
    let follow = query_as!(
        Follow,
        r"
        insert into follows
        (
            follower_id,
            following_id
        )
        values ($1, $2)
        on conflict (follower_id, following_id)
            do nothing
        returning *
        ",
        insert.follower_ap_user_id,
        insert.following_ap_user_id,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(follow)
}

pub async fn remove_if_exists(tx: &mut AppTx, insert: Insert) -> ResponseResult<()> {
    query!(
        r"
        delete from follows
        where follower_id = $1 and following_id = $2
        ",
        insert.follower_ap_user_id,
        insert.following_ap_user_id
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub struct ListBookmarkedUserFollowInsert {
    pub list_id: Uuid,
    pub bookmark_id: Uuid,
    pub followed_ap_user_id: Uuid,
    pub follow_id: Uuid,
}

pub async fn insert_list_follow(
    tx: &mut AppTx,
    insert: ListBookmarkedUserFollowInsert,
) -> ResponseResult<()> {
    query!(
        r"
        insert into list_bookmarked_user_follows
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

pub async fn remove_list_follow_by_list_id_if_exists(
    tx: &mut AppTx,
    list_id: Uuid,
) -> ResponseResult<()> {
    query!(
        r"
        delete from list_bookmarked_user_follows
        where list_id = $1
        ",
        list_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}
