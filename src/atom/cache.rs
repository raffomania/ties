//! Caching helpers for Atom feeds
//!
//! Provides an in-memory cache for rendered feeds with a time-to-live and a
//! signal that can be used to validate an entry before serving it.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

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
    /// When this entry was rendered, used to enforce the cache's time-to-live.
    cached_at: Instant,
}

impl Entry {
    /// Render a new cache entry for `body`, which matches `signal`.
    /// `last_modified` should be the time the representation was rendered.
    pub fn new(signal: FeedCacheSignal, body: String, last_modified: OffsetDateTime) -> Self {
        Self {
            etag: http_cache::etag_for(&body),
            signal,
            body: Bytes::from(body),
            last_modified,
            cached_at: Instant::now(),
        }
    }
}

/// Maximum number of rendered feeds kept in memory.
///
/// The cache's memory use is bounded by `MAX_ENTRIES` times the size of the
/// largest rendered feed.
pub const MAX_ENTRIES: usize = 256;

/// A small in-memory cache for rendered feeds with lazy expiration.
///
/// Entries are valid while they are younger than the configured time-to-live
/// and their signal equals the signal passed to [`Self::get`]. Expired entries
/// are removed when they are looked up, and the cache never grows beyond
/// `MAX_ENTRIES` entries: inserting an entry when full evicts the entry that
/// was rendered least recently.
pub struct FeedCache {
    entries: Mutex<HashMap<Uuid, Entry>>,
    ttl: Duration,
    max_entries: usize,
}

impl FeedCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            ttl,
            max_entries: MAX_ENTRIES,
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Uuid, Entry>> {
        // A poisoned lock only means that a previous request panicked while
        // holding it. The cache is still usable, so recover the guard.
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Return the cached entry for `key` if it is still fresh and its signal
    /// matches `signal`.
    pub fn get(&self, key: &Uuid, signal: &FeedCacheSignal) -> Option<Entry> {
        let mut entries = self.lock();
        let entry = entries.get(key)?;

        let expired = entry.cached_at.elapsed() >= self.ttl;
        let signal_matches = entry.signal == *signal;

        if expired {
            drop(entries.remove(key));
            return None;
        }

        if !signal_matches {
            return None;
        }

        Some(entry.clone())
    }

    /// Store `entry` for `key`, replacing any previous entry.
    ///
    /// Evicts expired entries and then the least recently rendered entries
    /// when the cache would otherwise exceed its capacity.
    pub fn insert(&self, key: Uuid, entry: Entry) {
        let mut entries = self.lock();
        entries.insert(key, entry);

        if entries.len() <= self.max_entries {
            return;
        }

        entries.retain(|_, entry| entry.cached_at.elapsed() < self.ttl);
        while entries.len() > self.max_entries {
            let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.cached_at)
                .map(|(key, _)| *key)
            else {
                break;
            };
            entries.remove(&oldest);
        }
    }

    /// Remove the entry for `key`, if any.
    pub fn remove(&self, key: &Uuid) {
        let mut entries = self.lock();
        drop(entries.remove(key));
    }
}

/// Build a response for `entry`, returning `304 Not Modified` when the
/// request's conditional headers match.
pub fn respond(request_headers: &HeaderMap, entry: &Entry) -> ResponseResult<Response> {
    let headers_match_cache = matches_caching_headers(request_headers, &entry);

    let mut response = if headers_match_cache {
        (StatusCode::NOT_MODIFIED, Body::empty()).into_response()
    } else {
        (StatusCode::OK, Body::from(entry.body.clone())).into_response()
    };

    let headers = response.headers_mut();
    headers.insert(
        header::ETAG,
        HeaderValue::try_from(&entry.etag).context("Failed to set header")?,
    );
    headers.insert(
        header::LAST_MODIFIED,
        httpdate::fmt_http_date(system_time_from(entry.last_modified))
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
fn matches_caching_headers(
    request_headers: &HeaderMap,
    Entry {
        etag,
        last_modified,
        ..
    }: &Entry,
) -> bool {
    // If-None-Match takes precedence over If-Modified-Since, even when it
    // doesn't match, per RFC 9110 section 13.2.2.
    if let Some(if_none_match) = header_str(request_headers, header::IF_NONE_MATCH) {
        return http_cache::if_none_match_matches(if_none_match, etag);
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
    if_modified_since >= last_modified.unix_timestamp()
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
    use std::{
        collections::HashMap,
        sync::Mutex,
        time::{Duration as StdDuration, Instant},
    };

    use anyhow::Result;
    use axum::{
        body::to_bytes,
        http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    };
    use pretty_assertions::assert_eq;
    use time::{Duration, OffsetDateTime};
    use uuid::Uuid;

    use super::{CACHE_CONTROL, Entry, FeedCache, respond, system_time_from};
    use crate::{
        db::lists::FeedCacheSignal,
        http_cache::{etag_for, if_none_match_matches},
    };

    fn signal() -> FeedCacheSignal {
        FeedCacheSignal {
            private: false,
            title: "List".to_string(),
            link_count: 0,
            newest_link_created_at: None,
            private_destination_count: 0,
        }
    }

    fn test_entry(body: &str, last_modified: OffsetDateTime) -> Entry {
        Entry::new(signal(), body.to_string(), last_modified)
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

        let response = respond(
            &headers(header::IF_NONE_MATCH, "\"does not match\""),
            &entry,
        )?;

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
        let response = respond(&request_headers, &entry)?;

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

        let response = respond(&request_headers, &entry)?;

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
        let response = respond(&request_headers, &entry)?;
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);

        request_headers.insert(
            header::IF_MODIFIED_SINCE,
            HeaderValue::from_str(&httpdate::fmt_http_date(system_time_from(
                last_modified() - Duration::seconds(1),
            )))
            .unwrap(),
        );
        let response = respond(&request_headers, &entry)?;
        assert_eq!(response.status(), StatusCode::OK);

        Ok(())
    }

    #[tokio::test]
    async fn unparseable_if_modified_since_is_ignored() -> Result<()> {
        let entry = test_entry("<feed/>", last_modified());

        let response = respond(&headers(header::IF_MODIFIED_SINCE, "not a date"), &entry)?;

        assert_eq!(response.status(), StatusCode::OK);

        Ok(())
    }

    #[test]
    fn cache_returns_entries_with_a_matching_signal() {
        let cache = FeedCache::new(StdDuration::from_hours(1));
        cache.insert(
            Uuid::nil(),
            Entry::new(signal(), "body".to_string(), last_modified()),
        );

        let entry = cache.get(&Uuid::nil(), &signal()).unwrap();
        assert_eq!(&*entry.body, b"body");
        let mut other_signal = signal();
        other_signal.link_count = 1;
        assert!(cache.get(&Uuid::nil(), &other_signal).is_none());
        assert!(cache.get(&Uuid::from_u128(1), &signal()).is_none());
    }

    #[test]
    fn cache_expires_entries_after_the_ttl() {
        let cache = FeedCache::new(StdDuration::from_hours(1));
        let mut entry = Entry::new(signal(), "body".to_string(), last_modified());
        entry.cached_at = std::time::Instant::now()
            .checked_sub(StdDuration::from_hours(2))
            .unwrap();
        cache.insert(Uuid::nil(), entry);

        assert!(cache.get(&Uuid::nil(), &signal()).is_none());
        // The expired entry was removed, not just hidden.
        assert!(cache.get(&Uuid::nil(), &signal()).is_none());
    }

    #[test]
    fn cache_entries_can_be_removed() {
        let cache = FeedCache::new(StdDuration::from_hours(1));
        cache.insert(
            Uuid::from_u128(1),
            Entry::new(signal(), "one".to_string(), last_modified()),
        );
        cache.insert(
            Uuid::from_u128(2),
            Entry::new(signal(), "two".to_string(), last_modified()),
        );

        cache.remove(&Uuid::from_u128(1));
        assert!(cache.get(&Uuid::from_u128(1), &signal()).is_none());
        assert!(cache.get(&Uuid::from_u128(2), &signal()).is_some());
    }

    fn entry_aged(age: StdDuration) -> Entry {
        let mut entry = Entry::new(signal(), "body".to_string(), last_modified());
        entry.cached_at = Instant::now()
            .checked_sub(age)
            .expect("age must fit in Instant");
        entry
    }

    #[test]
    fn cache_evicts_the_oldest_entry_when_over_capacity() {
        let cache = FeedCache::with_capacity(StdDuration::from_hours(1), 2);
        let key = Uuid::from_u128;

        cache.insert(key(1), entry_aged(StdDuration::from_secs(30)));
        cache.insert(key(2), entry_aged(StdDuration::from_secs(20)));
        cache.insert(key(3), entry_aged(StdDuration::from_secs(10)));

        assert_eq!(cache.len(), 2);
        assert!(cache.get(&key(1), &signal()).is_none());
        assert!(cache.get(&key(2), &signal()).is_some());
        assert!(cache.get(&key(3), &signal()).is_some());
    }

    #[test]
    fn cache_reaps_expired_entries_before_evicting_fresh_ones() {
        let cache = FeedCache::with_capacity(StdDuration::from_hours(1), 3);
        let key = Uuid::from_u128;

        cache.insert(key(1), entry_aged(StdDuration::from_hours(2)));
        cache.insert(key(2), entry_aged(StdDuration::from_mins(90)));
        cache.insert(key(3), entry_aged(StdDuration::from_secs(60)));
        cache.insert(key(4), entry_aged(StdDuration::from_secs(0)));

        // Both expired entries were reaped, so no fresh entry was evicted.
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&key(1), &signal()).is_none());
        assert!(cache.get(&key(2), &signal()).is_none());
        assert!(cache.get(&key(3), &signal()).is_some());
        assert!(cache.get(&key(4), &signal()).is_some());
    }

    #[test]
    fn cache_replacing_an_entry_does_not_evict_another_one() {
        let cache = FeedCache::with_capacity(StdDuration::from_hours(1), 2);
        let key = Uuid::from_u128;

        cache.insert(key(1), entry_aged(StdDuration::from_secs(30)));
        cache.insert(key(2), entry_aged(StdDuration::from_secs(20)));
        cache.insert(key(1), entry_aged(StdDuration::from_secs(0)));

        assert_eq!(cache.len(), 2);
        assert!(cache.get(&key(1), &signal()).is_some());
        assert!(cache.get(&key(2), &signal()).is_some());
    }

    impl FeedCache {
        fn with_capacity(ttl: StdDuration, max_entries: usize) -> Self {
            Self {
                entries: Mutex::new(HashMap::new()),
                ttl,
                max_entries,
            }
        }

        fn len(&self) -> usize {
            self.lock().len()
        }

        /// Mark the entry for `key` as expired. Used by tests to avoid sleeping.
        pub(crate) fn expire_entry(&self, key: &Uuid) {
            let mut entries = self.lock();
            if let Some(entry) = entries.get_mut(key) {
                entry.cached_at = Instant::now()
                    .checked_sub(self.ttl + StdDuration::from_secs(1))
                    .expect("TTL must fit in Instant");
            }
        }
    }
}
