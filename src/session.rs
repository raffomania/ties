use anyhow::Context as _;
use axum::{
    extract::FromRequestParts,
    http::{HeaderValue, request::Parts},
};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    authentication::AuthUser,
    db::{self, AppTx, User, sessions::Session},
    response_error::ResponseResult,
    server::AppState,
};

pub const COOKIE_NAME: &str = "ties-session";
const COOKIE_ATTRIBUTES: &str = "Path=/; HttpOnly; SameSite=Lax; Secure";
const SESSION_EXPIRY_DURATION: Duration = Duration::weeks(2);

impl Session {
    pub(crate) fn new() -> Session {
        let key = Uuid::new_v4().to_string();
        let expires_at = OffsetDateTime::now_utc() + SESSION_EXPIRY_DURATION;
        Session {
            key,
            contents: db::sessions::Contents::default(),
            expires_at,
        }
    }

    pub async fn rotate(self, tx: &mut AppTx) -> ResponseResult<Session> {
        db::sessions::delete(tx, self).await?;
        Ok(Session::new())
    }

    /// Persist current state to the database and return a `Set-Cookie` header
    /// for the current session key.
    pub async fn persist(&mut self, tx: &mut AppTx) -> ResponseResult<HeaderValue> {
        crate::db::sessions::upsert(tx, self).await?;
        set_cookie_header(&self.key)
    }

    pub async fn persist_logged_in_user(
        self,
        tx: &mut AppTx,
        user: &User,
    ) -> ResponseResult<HeaderValue> {
        let mut session = self.rotate(tx).await?;
        session.contents.auth_user = Some(AuthUser {
            user_id: user.id,
            ap_user_id: user.ap_user_id,
        });
        session.persist(tx).await
    }
}

pub fn set_cookie_header(key: &str) -> ResponseResult<HeaderValue> {
    Ok(HeaderValue::try_from(format!(
        "{}={}; {}; Max-Age={}",
        COOKIE_NAME,
        key,
        COOKIE_ATTRIBUTES,
        SESSION_EXPIRY_DURATION.whole_seconds(),
    ))
    .context("Failed to create cookie header value")?)
}

pub fn clear_cookie_header() -> ResponseResult<HeaderValue> {
    Ok(
        HeaderValue::try_from(format!("{COOKIE_NAME}=; Max-Age=0; {COOKIE_ATTRIBUTES}"))
            .context("Failed to create cookie header value")?,
    )
}

impl FromRequestParts<AppState> for Session {
    type Rejection = crate::response_error::ResponseError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cookie_header = parts
            .headers
            .get(axum::http::header::COOKIE)
            .and_then(|h| h.to_str().ok())
            .map(ToOwned::to_owned);

        let session_key = cookie_header.as_deref().and_then(extract_session_key);

        let state_inner = if let Some(key) = session_key {
            match crate::db::sessions::load(&state.pool, &key.to_string()).await {
                Ok(Some(loaded)) => loaded,
                Ok(None) => Session::new(),
                Err(e) => return Err(e.into()),
            }
        } else {
            Session::new()
        };

        Ok(state_inner)
    }
}

fn extract_session_key(cookie_header: &str) -> Option<Uuid> {
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some((name, value)) = cookie.split_once('=')
            && name.trim() == COOKIE_NAME
            && let Ok(uuid) = value.trim().parse::<Uuid>()
        {
            return Some(uuid);
        }
    }
    None
}
