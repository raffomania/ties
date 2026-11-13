//! Caching helpers for Atom feeds
//!
//! Provides an in-memory cache for rendered feeds with a time-to-live and a
//! signal that can be used to validate an entry before serving it.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use axum::{
    body::{Body, Bytes},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{db::lists::FeedCacheSignal, http_cache, response_error::ResponseResult};

/// Value of the `Cache-Control` header sent with cached responses.
/// It's okay to send 'public' here: we only serve public lists, and if a list
/// changes from public to private, any external caching proxies will only have
/// the previously-public version of the list.
pub const CACHE_CONTROL: HeaderValue = HeaderValue::from_static("public, max-age=300");

const CONTENT_TYPE: HeaderValue = HeaderValue::from_static("application/atom+xml; charset=utf-8");

/// A cached response body together with the information needed to revalidate
/// it.
#[derive(Clone)]
pub struct Entry {
    /// The signal at the time the entry was rendered. An entry is only served
    /// while the current signal matches this value.
    signal: FeedCacheSignal,
    /// The rendered response body.
    body: Bytes,
    /// Strong entity tag for [`Self::body`].
    etag: String,
    /// The time the representation was rendered, used for `Last-Modified`.
    last_modified: OffsetDateTime,
}

impl Entry {
    /// Render a new cache entry for `body`, which matches `signal`, stamped
    /// with an explicit `last_modified` time.
    pub fn new_at(signal: FeedCacheSignal, body: String, last_modified: OffsetDateTime) -> Self {
        Self {
            etag: http_cache::etag_for(&body),
            signal,
            body: Bytes::from(body),
            last_modified,
        }
    }

    /// Build a response for `entry`, returning `304 Not Modified` when the
    /// request's conditional headers match.
    pub fn respond(&self, request_headers: &HeaderMap) -> ResponseResult<Response> {
        let headers_match_cache = self.matches_caching_headers(request_headers);

        let mut response = if headers_match_cache {
            (StatusCode::NOT_MODIFIED, Body::empty()).into_response()
        } else {
            (StatusCode::OK, Body::from(self.body.clone())).into_response()
        };

        let headers = response.headers_mut();
        headers.insert(
            header::ETAG,
            HeaderValue::try_from(&self.etag).context("Failed to set header")?,
        );
        headers.insert(
            header::LAST_MODIFIED,
            httpdate::fmt_http_date(system_time_from(self.last_modified))
                .try_into()
                .context("Failed to set header")?,
        );
        headers.insert(header::CACHE_CONTROL, CACHE_CONTROL);
        if !headers_match_cache {
            headers.insert(header::CONTENT_TYPE, CONTENT_TYPE);
        }

        Ok(response)
    }

    /// Compare request headers to the given etag and last_modified value
    /// and check if the entry was changed since the last time the client saw it.
    fn matches_caching_headers(&self, request_headers: &HeaderMap) -> bool {
        // If-None-Match takes precedence over If-Modified-Since, even when it
        // doesn't match, per RFC 9110 section 13.2.2.
        if let Some(if_none_match) = header_str(request_headers, header::IF_NONE_MATCH) {
            return http_cache::if_none_match_matches(if_none_match, &self.etag);
        }

        let Some(if_modified_since) = header_str(request_headers, header::IF_MODIFIED_SINCE) else {
            return false;
        };
        let Ok(if_modified_since) = httpdate::parse_http_date(if_modified_since) else {
            return false;
        };
        let Some(if_modified_since) = system_time_to_seconds(if_modified_since) else {
            return false;
        };

        // HTTP dates have one-second resolution, so compare against the truncated
        // render time.
        if_modified_since >= self.last_modified.unix_timestamp()
    }
}

/// Maximum number of rendered feeds kept in memory.
///
/// The bound is best-effort, so the cache's memory use is roughly
/// `MAX_ENTRIES` times the size of the largest rendered feed.
pub const MAX_ENTRIES: u64 = 256;

/// A small in-memory cache for rendered feeds with a time-to-live.
///
/// An entry is served while it is younger than the configured time-to-live
/// and its signal equals the signal passed to [`Self::get`]. Expired entries
/// stop being served immediately even though their memory is reclaimed
/// later, and inserting entries beyond [`MAX_ENTRIES`] evicts the entries
/// that are used least often.
#[derive(Clone)]
pub struct FeedCache {
    entries: moka::sync::Cache<Uuid, Entry>,
}

impl FeedCache {
    /// Create a cache that serves entries for at most `ttl`.
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: moka::sync::Cache::builder()
                .max_capacity(MAX_ENTRIES)
                .time_to_live(ttl)
                .build(),
        }
    }

    /// Return the cached entry for `key` if it is still fresh and its signal
    /// matches `signal`.
    pub fn get(&self, key: &Uuid, signal: &FeedCacheSignal) -> Option<Entry> {
        self.entries
            .get(key)
            .filter(|entry| entry.signal == *signal)
    }

    /// Store `entry` for `key`, replacing any previous entry and restarting
    /// its time-to-live.
    pub fn insert(&self, key: Uuid, entry: Entry) {
        self.entries.insert(key, entry);
    }

    /// Remove the entry for `key`, if any.
    ///
    /// The entry stops being served immediately; its memory is reclaimed
    /// later. This only frees memory: a stale entry can never be served
    /// anyway, because its signal would no longer match.
    pub fn invalidate(&self, key: &Uuid) {
        self.entries.invalidate(key);
    }
}

fn header_str(headers: &HeaderMap, name: HeaderName) -> Option<&str> {
    headers.get(name)?.to_str().ok()
}

/// Get unix timestamp from a datetime.
/// Returns UNIX_EPOCH for earlier dates.
fn system_time_from(date_time: OffsetDateTime) -> SystemTime {
    let seconds = date_time.unix_timestamp();
    match u64::try_from(seconds) {
        Ok(seconds) => UNIX_EPOCH + Duration::from_secs(seconds),
        // time before the Unix epoch
        Err(_) => UNIX_EPOCH,
    }
}

/// Returns the unix timestamp as seconds.
/// Returns 0 for dates before UNIX_EPOCH.
fn system_time_to_seconds(system_time: SystemTime) -> Option<i64> {
    match system_time.duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_secs()).ok(),
        // time before the Unix epoch
        Err(_) => Some(0),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration as StdDuration;

    use anyhow::Result;
    use axum::{
        body::to_bytes,
        http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    };
    use pretty_assertions::assert_eq;
    use time::{Duration, OffsetDateTime};
    use uuid::Uuid;

    use super::{CACHE_CONTROL, Entry, FeedCache, MAX_ENTRIES, system_time_from};
    use crate::{
        db::lists::FeedCacheSignal,
        http_cache::{etag_for, if_none_match_matches},
    };

    fn signal() -> FeedCacheSignal {
        FeedCacheSignal {
            title: "List".to_string(),
            link_count: 0,
            newest_link_created_at: None,
            private_destination_count: 0,
        }
    }

    fn test_entry(body: &str, last_modified: OffsetDateTime) -> Entry {
        Entry::new_at(signal(), body.to_string(), last_modified)
    }

    fn last_modified() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_600_000_000).unwrap()
    }

    fn headers(name: HeaderName, value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(name, HeaderValue::from_static(value));
        headers
    }

    async fn body_string(response: axum::response::Response) -> String {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    #[test]
    fn etag_is_a_quoted_hash_of_the_body() {
        let etag = etag_for("<feed/>");

        assert!(etag.starts_with('"'));
        assert!(etag.ends_with('"'));
        assert_eq!(etag.len(), 18);
        assert_eq!(etag, etag_for("<feed/>"));
        assert_ne!(etag, etag_for("<feed />"));
    }

    #[test]
    fn if_none_match_handles_lists_wildcards_and_weak_tags() {
        let etag = "\"abc\"";

        for header_value in [
            "\"abc\"",
            "W/\"abc\"",
            "\"other\", \"abc\"",
            "\"other\", W/\"abc\"",
            "*",
            "  \"abc\"  ",
        ] {
            assert!(
                if_none_match_matches(header_value, etag),
                "expected {header_value:?} to match {etag:?}"
            );
        }

        for header_value in ["\"other\"", "\"ab\"", "garbage", ""] {
            assert!(
                !if_none_match_matches(header_value, etag),
                "expected {header_value:?} not to match {etag:?}"
            );
        }
    }

    #[tokio::test]
    async fn matching_if_none_match_returns_304_with_revalidation_headers() -> Result<()> {
        let entry = test_entry("<feed/>", last_modified());

        let response = entry.respond(&headers(header::IF_NONE_MATCH, "\"does not match\""))?;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::ETAG).unwrap(),
            HeaderValue::from_str(&entry.etag).unwrap()
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            CACHE_CONTROL
        );
        assert_eq!(body_string(response).await, "<feed/>");

        let mut request_headers = HeaderMap::new();
        request_headers.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(&entry.etag).unwrap(),
        );
        let response = entry.respond(&request_headers)?;

        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(
            response.headers().get(header::ETAG).unwrap(),
            HeaderValue::from_str(&entry.etag).unwrap()
        );
        assert_eq!(
            response.headers().get(header::LAST_MODIFIED).unwrap(),
            HeaderValue::from_str(&httpdate::fmt_http_date(system_time_from(
                entry.last_modified
            )))
            .unwrap()
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            CACHE_CONTROL
        );
        assert_eq!(body_string(response).await, "");

        Ok(())
    }

    #[tokio::test]
    async fn if_none_match_takes_precedence_over_if_modified_since() -> Result<()> {
        let entry = test_entry("<feed/>", last_modified());
        let mut request_headers = headers(header::IF_NONE_MATCH, "\"other\"");
        request_headers.insert(
            header::IF_MODIFIED_SINCE,
            HeaderValue::from_str(&httpdate::fmt_http_date(system_time_from(
                last_modified() + Duration::hours(1),
            )))
            .unwrap(),
        );

        let response = entry.respond(&request_headers)?;

        assert_eq!(response.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn if_modified_since_compares_at_second_resolution() -> Result<()> {
        let entry = test_entry("<feed/>", last_modified() + Duration::milliseconds(900));
        // The header can only represent whole seconds, so it is one second
        // before the entry's exact modification time.
        let if_modified_since = httpdate::fmt_http_date(system_time_from(last_modified()));

        let mut request_headers = HeaderMap::new();
        request_headers.insert(
            header::IF_MODIFIED_SINCE,
            HeaderValue::from_str(&if_modified_since).unwrap(),
        );
        let response = entry.respond(&request_headers)?;
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);

        request_headers.insert(
            header::IF_MODIFIED_SINCE,
            HeaderValue::from_str(&httpdate::fmt_http_date(system_time_from(
                last_modified() - Duration::seconds(1),
            )))
            .unwrap(),
        );
        let response = entry.respond(&request_headers)?;
        assert_eq!(response.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn unparseable_if_modified_since_is_ignored() -> Result<()> {
        let entry = test_entry("<feed/>", last_modified());

        let response = entry.respond(&headers(header::IF_MODIFIED_SINCE, "not a date"))?;

        assert_eq!(response.status(), StatusCode::OK);

        Ok(())
    }

    #[test]
    fn cache_returns_entries_with_a_matching_signal() {
        let cache = FeedCache::new(StdDuration::from_hours(1));
        cache.insert(Uuid::nil(), test_entry("body", last_modified()));

        let entry = cache.get(&Uuid::nil(), &signal()).unwrap();
        assert_eq!(&*entry.body, b"body");
        let mut other_signal = signal();
        other_signal.link_count = 1;
        assert!(cache.get(&Uuid::nil(), &other_signal).is_none());
        assert!(cache.get(&Uuid::from_u128(1), &signal()).is_none());
    }

    #[test]
    fn cache_expires_entries_after_the_ttl() {
        let cache = FeedCache::new(StdDuration::from_millis(5));
        cache.insert(Uuid::nil(), test_entry("body", last_modified()));

        std::thread::sleep(StdDuration::from_millis(50));

        assert!(cache.get(&Uuid::nil(), &signal()).is_none());
        // The entry stays hidden even though its memory is reclaimed later.
        assert!(cache.get(&Uuid::nil(), &signal()).is_none());
    }

    #[test]
    fn cache_entries_can_be_removed() {
        let cache = FeedCache::new(StdDuration::from_hours(1));
        cache.insert(Uuid::from_u128(1), test_entry("one", last_modified()));
        cache.insert(Uuid::from_u128(2), test_entry("two", last_modified()));

        cache.invalidate(&Uuid::from_u128(1));
        assert!(cache.get(&Uuid::from_u128(1), &signal()).is_none());
        assert!(cache.get(&Uuid::from_u128(2), &signal()).is_some());
    }

    #[test]
    fn cache_stays_within_max_entries() {
        let cache = FeedCache::new(StdDuration::from_hours(1));

        for i in 0..MAX_ENTRIES + 64 {
            cache.insert(
                Uuid::from_u128(u128::from(i)),
                test_entry("body", last_modified()),
            );
        }

        // Eviction is best-effort, so let the cache finish its pending work.
        cache.entries.run_pending_tasks();
        assert!(cache.entries.entry_count() <= MAX_ENTRIES);
    }
}
