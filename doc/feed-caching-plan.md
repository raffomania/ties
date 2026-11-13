# Plan for Caching Headers on the List Feed Route

Technical plan for adding HTTP caching to `GET /lists/{list_id}/feed.atom`
(`get_feed` in `src/routes/lists.rs`).

## Requirements

- Serve stale data for up to 5 minutes on the client side.
- Support both `ETag`/`If-None-Match` and `Last-Modified`/`If-Modified-Since`.
- No database schema changes (so no `updated_at` columns).
- No proxies; everything happens in our Rust codebase.
- Skip database work where possible using in-memory state, without serving
  stale data forever.
- New links must show up quickly. Bookmark edits and link deletions are rare
  and may take up to an hour to propagate.
- Ties is self-hosted, so self-hosted scale is the design target.

## Constraints found while planning

- There are no `updated_at` columns, only `created_at`. Feed content depends
  on: list title (editable), owner username, the set of links (add *and*
  remove), and bookmark titles/URLs (editable). A validator derived from
  timestamps alone cannot detect edits.
- Privacy cannot be cached: `edit_private` can flip a list to private at any
  time, and serving a cached feed after that would leak it publicly.
- Library survey: no suitable standalone library exists. `tower-http` has no
  conditional-GET middleware for dynamic handlers (open issue since 2023);
  `tower-etag-cache` does body hashing plus response caching but has no
  signal-based invalidation; the good implementations are welded into
  frameworks (loco, rustango). Hand-rolling is ~150 lines plus tests.
- `httpdate` is already in the dependency tree via hyper. axum, tower and the
  `http` crate expose no HTTP-date parsing themselves. `chrono`/`time` would
  mean hand-rolling RFC 9110 date parsing (three formats) to save a zero-cost
  dependency line.

## Design: signal-validated in-memory cache

Instead of a pure TTL cache (which would also delay new links), the cache is
validated by a cheap per-request signal query. The 1-hour TTL is only a
backstop for changes the signal cannot see.

Signal query (one indexed query per request):

```sql
select lists.title,
       count(links.id) as link_count,
       max(links.created_at) as newest_link_created_at,
       count(dest_lists.id) filter (where dest_lists.private)
           as private_destination_count
from lists
left join links on links.src_list_id = lists.id
left join lists as dest_lists on dest_lists.id = links.dest_list_id
where lists.id = $1 and not lists.private
group by lists.id
```

The `not lists.private` filter means private lists never produce a signal, so
access control lives in exactly one place: no signal -> `404`, and the route
never renders or serves them.

Detected instantly: new links, link deletions (count changes), list title
edits (title is in the row, so including it is free), private flips, list
deletion, linked lists becoming private. Only detected after TTL expiry:
bookmark title/URL edits, username changes.

### Request flow in the rewritten `get_feed`

1. Run the signal query. No row (missing or private) -> `404` and evict the
   cache key.
2. Cache hit requires: entry exists, is younger than the TTL, and stored
   signal equals the current signal. On a hit, serve from the cache.
3. On a miss, `render_and_cache` does the full render (`by_id_with_metadata` +
   `list_by_list` + `Feed::for_list` + `to_xml`), stores
   `{signal, body, etag, last_modified}`, then serves. Storing only happens
   on a miss, so a hit never restarts the TTL.
4. Serving (both paths) evaluates conditional headers:
   `If-None-Match` match -> `304`; otherwise
   `If-Modified-Since >= last_modified` -> `304`; otherwise `200` with the
   body. All responses carry `ETag`, `Last-Modified` and
   `Cache-Control: public, max-age=300`.

## Key decisions

- **ETag**: strong, `"{:016x}"` of a `DefaultHasher` hash over the rendered
  XML. Deterministic across restarts, no new dependencies, correct by
  construction since it hashes the actual response bytes.
- **Last-Modified**: the time the cached representation was rendered,
  truncated to seconds (HTTP-date granularity). Every re-render (signal
  change or TTL expiry) produces a newer value, so clients that send only
  `If-Modified-Since` still receive changed content after a re-render.
  Trade-off: such a client gets a `200` after each re-render even when the
  body is unchanged; ETag clients still get `304`. `If-None-Match` still
  takes precedence over `If-Modified-Since` per RFC 9110 section 13.2.2.
- **Cache store**: `moka::sync::Cache<Uuid, Entry>` behind our own
  `FeedCache` newtype, configured with a 1-hour TTL and `MAX_ENTRIES = 256`
  (best-effort eviction of the least frequently used entries). Moka was
  already in the dependency tree via `activitypub_federation`, so this adds no
  new crates. The newtype keeps `AppState` (`feed_cache`, no `Arc` needed:
  moka caches are cloneable handles) and the signal-validated `get` stable, so
  a future store swap stays a one-file change. One entry per list; entries are
  evicted on 404 and replaced on re-render. Concurrent misses may both render;
  last write wins, which is benign. Entries are only inserted on a miss, so a
  cache hit never extends the TTL.
- **Date handling**: add `httpdate = "1"` as a direct dependency (already
  compiled as a transitive dependency of hyper, so zero new code). `time` is
  built without its `std` feature, so convert `OffsetDateTime` to and from
  `SystemTime` via `unix_timestamp()`.
- **Cache-Control**: `public, max-age=300`. Feeds are public-only (private
  lists return 404), so `public` is safe. Client-side staleness of 5 minutes
  is explicitly accepted.

## Expected behavior once shipped

- Feed readers polling at their usual interval revalidate cheaply (304s).
- New links appear on the first request after they are added (bounded only by
  the client's `max-age=300`).
- Bookmark edits and username changes propagate within the 1-hour TTL.
- Private flips and deletions take effect immediately.
- Server restarts lose the cache but keep ETags stable (content hash), so
  clients revalidate with one 200 and resume 304s.

## Testing

Integration tests in `src/tests/lists.rs` cover the caching behavior:

- `feed_sets_cache_headers_and_supports_conditional_requests`: headers are
  present and a matching `If-None-Match` returns `304`.
- `adding_a_link_invalidates_the_cached_feed`: a new link changes the `ETag`
  and body.
- `bookmark_edits_are_served_from_cache_until_the_entry_is_removed`: edits the
  signal cannot see are served stale until the entry is evicted.
- `bookmark_edits_are_served_after_the_ttl_expires`: an edit invisible to the
  signal is stale while the entry is fresh and appears once the TTL is
  simulated to have elapsed.
- `feed_revalidates_after_removing_the_newest_link`: removing the newest link
  and revalidating with the old `Last-Modified` must return `200` with the new
  body (not `304`).
- `private_lists_are_not_served_from_cache`: flipping a list private returns
  `404` and drops the cache entry.
- `private_linked_lists_are_removed_from_the_cached_feed`: a linked list
  becoming private disappears from the outer feed.
- `feed_of_unknown_list_returns_not_found`: unknown list ids return `404`.

`src/atom/cache.rs` has unit tests for the `ETag`, `Last-Modified` and
`FeedCache` behavior.

## Edge cases accepted

- Delete-and-insert within one database transaction could theoretically leave
  count and max(created_at) unchanged, hiding the change from the signal. The
  TTL backstop catches it within an hour.
- HTTP dates have one-second granularity; changes within the same second as
  the last render can be missed by `If-Modified-Since`-only clients. Standard
  practice, also accepted by nginx and others.
- Expired entries for lists that are never requested again linger in memory
  until restart; bounded by the number of lists, which is fine at self-hosted
  scale.
