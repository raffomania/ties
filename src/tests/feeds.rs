use std::time::Duration;

use axum::http::{StatusCode, header};
use rstest::rstest;
use uuid::Uuid;

use crate::{
    atom::cache,
    db::{self, bookmarks::UpdateBookmark},
    route_paths,
    tests::util::{request_builder::TestResponse, test_app::TestApp},
};

const SHORT_FEED_CACHE_TTL: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug)]
enum FeedEndpoint {
    List,
    User,
}

impl FeedEndpoint {
    fn url(self, user: &db::User, list: &db::List) -> String {
        match self {
            Self::List => route_paths::list_feed(list.id),
            Self::User => route_paths::local_user_feed(&user.username),
        }
    }
}

async fn read_feed_response(response: TestResponse) -> (String, String, String) {
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/atom+xml; charset=utf-8"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        cache::CACHE_CONTROL
    );

    let etag = response.header_string(header::ETAG);
    let last_modified = response.header_string(header::LAST_MODIFIED);
    let body = response.body_string().await;

    (etag, last_modified, body)
}

#[rstest]
#[case::list(FeedEndpoint::List)]
#[case::user(FeedEndpoint::User)]
#[test_log::test(tokio::test)]
async fn feed_sets_cache_headers_and_supports_conditional_requests(
    #[case] endpoint: FeedEndpoint,
) -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let bookmark = app
        .create_bookmark(&user, "https://example.com", "Example")
        .await;
    let list = app.create_list(&user, "Feed list").await;
    app.create_link(&user, list.id, bookmark.id).await;

    let url = endpoint.url(&user, &list);
    let (etag, last_modified, body) = read_feed_response(app.req().get(&url).await).await;
    assert!(body.contains("<title>Example</title>"));

    // Check that sending etag or last modified both result in a not modified
    // response
    for (name, value) in [
        (header::IF_NONE_MATCH, &etag),
        (header::IF_MODIFIED_SINCE, &last_modified),
    ] {
        let response = app
            .req()
            .header(name, value.clone())
            .expect_status(StatusCode::NOT_MODIFIED)
            .get(&url)
            .await;

        assert_eq!(&response.header_string(header::ETAG), &etag);
        assert_eq!(
            &response.header_string(header::LAST_MODIFIED),
            &last_modified
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            cache::CACHE_CONTROL
        );
        assert_eq!(response.body_string().await, "");
    }

    Ok(())
}

#[rstest]
#[case::list(FeedEndpoint::List)]
#[case::user(FeedEndpoint::User)]
#[test_log::test(tokio::test)]
async fn adding_a_link_invalidates_cache(#[case] endpoint: FeedEndpoint) -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let list = app.create_list(&user, "Feed list").await;
    let url = endpoint.url(&user, &list);

    let first = app
        .create_bookmark(&user, "https://example.com/1", "First")
        .await;
    app.create_link(&user, list.id, first.id).await;

    let (first_etag, _, body) = read_feed_response(app.req().get(&url).await).await;
    assert!(body.contains("<title>First</title>"));

    let second = app
        .create_bookmark(&user, "https://example.com/2", "Second")
        .await;
    app.create_link(&user, list.id, second.id).await;

    let (second_etag, _, body) = read_feed_response(app.req().get(&url).await).await;
    assert_ne!(first_etag, second_etag);
    assert!(body.contains("<title>First</title>"));
    assert!(body.contains("<title>Second</title>"));

    Ok(())
}

#[rstest]
#[case::list(FeedEndpoint::List)]
#[case::user(FeedEndpoint::User)]
#[test_log::test(tokio::test)]
async fn bookmark_edits_are_served_from_cache_until_ttl_expires(
    #[case] endpoint: FeedEndpoint,
) -> anyhow::Result<()> {
    let mut app = TestApp::with_feed_cache_ttl(SHORT_FEED_CACHE_TTL).await;
    let user = app.create_test_user().await;
    let bookmark = app
        .create_bookmark(&user, "https://example.com", "Original title")
        .await;
    let list = app.create_list(&user, "Feed list").await;
    app.create_link(&user, list.id, bookmark.id).await;
    let url = endpoint.url(&user, &list);

    let (etag, _, body) = read_feed_response(app.req().get(&url).await).await;
    assert!(body.contains("<title>Original title</title>"));

    let mut tx = app.tx().await;
    db::bookmarks::update_local(
        &mut tx,
        bookmark.id,
        UpdateBookmark {
            title: Some("Updated title".to_string()),
        },
        user.ap_user_id,
    )
    .await?;
    tx.commit().await?;

    let body = app.req().get(&url).await.body_string().await;
    assert!(body.contains("<title>Original title</title>"));
    assert!(!body.contains("<title>Updated title</title>"));

    tokio::time::sleep(SHORT_FEED_CACHE_TTL + Duration::from_millis(200)).await;

    let body = app
        .req()
        .header(header::IF_NONE_MATCH, etag)
        .expect_status(StatusCode::OK)
        .get(&url)
        .await
        .body_string()
        .await;
    assert!(body.contains("<title>Updated title</title>"));
    assert!(!body.contains("<title>Original title</title>"));

    Ok(())
}

#[rstest]
#[case::list(route_paths::list_feed(Uuid::new_v4()))]
#[case::user(route_paths::local_user_feed("nonexistent"))]
#[test_log::test(tokio::test)]
async fn feed_of_unknown_target_returns_not_found(#[case] url: String) -> anyhow::Result<()> {
    let mut app = TestApp::new().await;

    app.req()
        .expect_status(StatusCode::NOT_FOUND)
        .get(&url)
        .await;

    Ok(())
}
