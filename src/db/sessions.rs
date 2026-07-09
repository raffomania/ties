use anyhow::Context;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, query, query_as, types::Json};
use time::OffsetDateTime;

use crate::{db::AppTx, session::SESSION_EXPIRY_DURATION};

/// Actual contents of the session without the metadata.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Contents {
    #[serde(default)]
    pub auth_user: Option<crate::authentication::AuthUser>,
    /// Temporarily stored while OIDC users are in the signup process and don't
    /// have a normal user record yet.
    #[serde(default)]
    pub oidc_user_info: Option<crate::oidc::AuthenticatedOidcUserInfo>,
    #[serde(default)]
    pub oidc_login_attempt: Option<crate::oidc::LoginAttempt>,
}

struct Row {
    session_key: String,
    contents: Json<Contents>,
    expires_at: OffsetDateTime,
}

pub async fn load(pool: &PgPool, key: &str) -> anyhow::Result<Option<Session>> {
    let now = OffsetDateTime::now_utc();
    let record = query_as!(
        Row,
        r#"
        select session_key, contents as "contents: Json<Contents>", expires_at
        from sessions
        where session_key = $1
            and expires_at > $2
        "#,
        key,
        now as OffsetDateTime,
    )
    .fetch_optional(pool)
    .await?;

    record
        .map(|record| {
            Ok(Session {
                key: record.session_key,
                contents: record.contents.0,
                expires_at: record.expires_at,
            })
        })
        .transpose()
}

#[derive(Debug)]
pub struct Session {
    pub key: String,
    pub contents: Contents,
    pub expires_at: OffsetDateTime,
}

pub async fn upsert(
    tx: &mut AppTx,
    Session {
        key,
        contents,
        expires_at,
    }: &Session,
) -> anyhow::Result<()> {
    let contents =
        serde_json::to_value(contents).context("Failed to serialize session contents")?;
    query!(
        "
        insert into sessions
        (session_key, contents, expires_at)
        values ($1, $2, $3)
        on conflict (session_key) do update set
            contents = $2,
            expires_at = $3
        ",
        key,
        contents,
        expires_at as &OffsetDateTime,
    )
    .execute(&mut **tx)
    .await
    .context("Failed to upsert session")?;

    Ok(())
}

pub async fn extend_expiry(tx: &mut AppTx, key: &str) -> anyhow::Result<()> {
    let expires_at = OffsetDateTime::now_utc() + SESSION_EXPIRY_DURATION;
    query!(
        "
        update sessions
        set expires_at = $2
        where session_key = $1
        ",
        key,
        expires_at as OffsetDateTime,
    )
    .execute(&mut **tx)
    .await
    .context("Failed to extend session expiry")?;

    Ok(())
}

pub async fn delete(tx: &mut AppTx, Session { key, .. }: Session) -> anyhow::Result<()> {
    query!(
        "
        delete from sessions
        where session_key = $1
        ",
        key
    )
    .execute(&mut **tx)
    .await
    .context("Failed to delete session")?;

    Ok(())
}

pub fn spawn_cleanup_task(pool: &PgPool) {
    let pool = pool.clone();
    tokio::spawn(async move {
        let interval = std::time::Duration::from_hours(6);
        let mut interval = tokio::time::interval(interval);
        interval.tick().await;
        loop {
            interval.tick().await;
            match delete_expired(&pool).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!("Deleted {count} expired sessions");
                    }
                }
                Err(e) => tracing::error!("Failed to delete expired sessions: {e:?}"),
            }
        }
    });
}

async fn delete_expired(pool: &PgPool) -> anyhow::Result<u64> {
    let now = OffsetDateTime::now_utc();
    let result = query!(
        "
        delete from sessions where expires_at <= $1
        ",
        now as OffsetDateTime
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
