use anyhow::anyhow;
use rand::distr::{Alphanumeric, SampleString};
use sqlx::{prelude::FromRow, query_as};
use time::OffsetDateTime;
use uuid::Uuid;

use super::AppTx;
use crate::response_error::ResponseResult;

#[expect(dead_code, reason = "Kept for reference on the DB schema")]
#[derive(FromRow, Debug)]
pub struct Invite {
    pub id: Uuid,

    pub created_at: OffsetDateTime,

    // user_id, not ap_user_id
    pub invited_by: Uuid,

    pub token: String,
}

pub const VALID_DURATION: time::Duration = time::Duration::hours(24);

pub async fn insert(tx: &mut AppTx, invited_by: Uuid) -> ResponseResult<Invite> {
    let token = Alphanumeric.sample_string(&mut rand::rng(), 20);
    let invite = query_as!(
        Invite,
        r#"
        insert into invites
            (invited_by, token)
        values
            ($1, $2)
        returning *
        "#,
        invited_by,
        token
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(invite)
}

pub async fn delete(tx: &mut AppTx, token: &str) -> ResponseResult<()> {
    let res = sqlx::query!(
        r#"
        delete from invites
        where token = $1
        "#,
        token
    )
    .execute(&mut **tx)
    .await?;

    if res.rows_affected() != 1 {
        return Err(anyhow!("Deleting invite affected less or more than 1 row").into());
    }

    Ok(())
}

pub async fn delete_expired(tx: &mut AppTx) -> ResponseResult<()> {
    let expires_at = time::OffsetDateTime::now_utc() - VALID_DURATION;
    sqlx::query!(
        r#"
        delete from invites
        where created_at < $1
        "#,
        expires_at
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn by_token(tx: &mut AppTx, token: &str) -> ResponseResult<Option<Invite>> {
    let created_after = time::OffsetDateTime::now_utc() - VALID_DURATION;
    let invite = query_as!(
        Invite,
        r#"
        select * from invites
        where token = $1
            and created_at > $2
        "#,
        token,
        created_after
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(invite)
}
