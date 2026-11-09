//! HTTP cache header helpers

use std::hash::{DefaultHasher, Hash, Hasher};

/// Compute a strong `ETag` for `body`.
///
/// The tag hashes the rendered bytes, so it changes whenever the response body
/// changes and stays the same across restarts for unchanged content.
pub fn etag_for(body: &str) -> String {
    let mut hasher = DefaultHasher::new();
    body.hash(&mut hasher);
    format!("\"{:016x}\"", hasher.finish())
}

/// Check whether an `If-None-Match` header matches `etag`.
///
/// Supports wildcards, comma-separated lists and weakly prefixed tags. Weak
/// comparison is used as required for `If-None-Match` by RFC 9110 section
/// 13.1.2.
pub fn if_none_match_matches(header_value: &str, etag: &str) -> bool {
    header_value.split(',').any(|candidate| {
        let candidate = candidate.trim();
        candidate == "*" || strip_weak(candidate) == strip_weak(etag)
    })
}

fn strip_weak(tag: &str) -> &str {
    tag.strip_prefix("W/").unwrap_or(tag)
}
