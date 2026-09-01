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

pub const SESSION_EXPIRY_DURATION: Duration = Duration::weeks(2);

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
    pub async fn persist(
        &mut self,
        tx: &mut AppTx,
        base_url: &url::Url,
    ) -> ResponseResult<HeaderValue> {
        crate::db::sessions::upsert(tx, self).await?;
        set_cookie_header(&self.key, base_url)
    }

    pub async fn persist_logged_in_user(
        self,
        tx: &mut AppTx,
        user: &User,
        base_url: &url::Url,
    ) -> ResponseResult<HeaderValue> {
        let mut session = self.rotate(tx).await?;
        session.contents.auth_user = Some(AuthUser {
            user_id: user.id,
            ap_user_id: user.ap_user_id,
        });
        session.persist(tx, base_url).await
    }

    pub fn expires_in(&self) -> Duration {
        self.expires_at - OffsetDateTime::now_utc()
    }
}

pub fn set_cookie_header(key: &str, base_url: &url::Url) -> ResponseResult<HeaderValue> {
    let secure_flag = if base_url.scheme() == "https" {
        "; Secure"
    } else {
        ""
    };
    Ok(HeaderValue::try_from(format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax{}; Max-Age={}",
        COOKIE_NAME,
        key,
        secure_flag,
        SESSION_EXPIRY_DURATION.whole_seconds(),
    ))
    .context("Failed to create cookie header value")?)
}

pub fn clear_cookie_header(base_url: &url::Url) -> ResponseResult<HeaderValue> {
    let secure_flag = if base_url.scheme() == "https" {
        "; Secure"
    } else {
        ""
    };
    Ok(HeaderValue::try_from(format!(
        "{COOKIE_NAME}=; Max-Age=0; Path=/; HttpOnly; SameSite=Lax{secure_flag}"
    ))
    .context("Failed to create cookie header value")?)
}

impl FromRequestParts<AppState> for Session {
    type Rejection = crate::response_error::ResponseError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cookie_headers: Vec<&str> = parts
            .headers
            .get_all(axum::http::header::COOKIE)
            .iter()
            .filter_map(|h| h.to_str().ok())
            .collect();

        let session_key = extract_session_key(cookie_headers);

        let state_inner = if let Some(key) = session_key {
            match crate::db::sessions::load(&state.pool, &key.to_string()).await {
                Ok(Some(loaded)) => loaded,
                Ok(None) => Session::new(),
                Err(e) => return Err(e.into()),
            }
        } else {
            Session::new()
        };

        // If a session has less time to live than this, extend its expiry on
        // activity. This way, we don't write to the DB on every
        // request.
        let minimum_expiry_duration = SESSION_EXPIRY_DURATION - Duration::days(1);
        if state_inner.expires_in() < minimum_expiry_duration {
            let mut tx = state.pool.begin().await?;
            db::sessions::extend_expiry(&mut tx, &state_inner.key).await?;
            tx.commit().await?;
        }

        Ok(state_inner)
    }
}

fn extract_session_key(cookie_headers: Vec<&str>) -> Option<Uuid> {
    for cookie_header in cookie_headers {
        for cookie in cookie_header.split(';') {
            if let Some((name, value)) = cookie.split_once('=')
                && name.trim() == COOKIE_NAME
                && let Ok(uuid) = value.trim().parse::<Uuid>()
            {
                return Some(uuid);
            }
        }
    }
    None
}
