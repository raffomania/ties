use sqlx::{prelude::FromRow, query};
use uuid::Uuid;

use crate::{db::AppTx, response_error::ResponseResult};

#[expect(dead_code, reason = "Kept for reference on the DB schema")]
#[derive(FromRow, Debug)]
pub struct Follow {
    pub id: Uuid,

    /// The user that is following
    pub follower_id: Uuid,
    /// The user being followed
    pub following_id: Uuid,
}

pub struct Insert {
    pub follower_id: Uuid,
    pub following_id: Uuid,
}

pub async fn upsert(tx: &mut AppTx, insert: Insert) -> ResponseResult<()> {
    // Don't return the follow because the `on conflict ... do nothing` won't return
    // anything on conflict
    query!(
        r"
        insert into follows
        (
            follower_id,
            following_id
        )
        values ($1, $2)
        on conflict (follower_id, following_id)
            do nothing
        ",
        insert.follower_id,
        insert.following_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn remove(tx: &mut AppTx, insert: Insert) -> ResponseResult<()> {
    query!(
        r"
        delete from follows
        where follower_id = $1 and following_id = $2
        ",
        insert.follower_id,
        insert.following_id
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}
