use axum::http::{StatusCode, header};
use uuid::Uuid;

use crate::{
    atom::cache,
    db::{self, bookmarks::UpdateBookmark},
    forms::links::CreateLink,
    tests::util::test_app::TestApp,
};

async fn create_link(
    app: &TestApp,
    user: &db::User,
    src: Uuid,
    dest: Uuid,
) -> anyhow::Result<db::links::Link> {
    let mut tx = app.tx().await;
    let link =
        db::links::insert(&mut tx, user.id, user.ap_user_id, CreateLink { src, dest }).await?;
    tx.commit().await?;
    Ok(link)
}

#[test_log::test(tokio::test)]
async fn get_create_list() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    app.create_test_user().await;
    app.login_test_user().await;

    let create_list = app.req().get("/lists/create").await.test_page().await;

    insta::assert_snapshot!(create_list.dom.find("main").htmls());

    Ok(())
}

#[test_log::test(tokio::test)]
async fn feed_sets_cache_headers_and_supports_conditional_requests() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let bookmark = app
        .create_bookmark(&user, "https://example.com", "Example")
        .await;
    let list = app.create_list(&user, "Feed list").await;
    create_link(&app, &user, list.id, bookmark.id).await?;

    let url = list.feed_path();
    let response = app.req().get(&url).await;

    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/atom+xml; charset=utf-8"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        cache::CACHE_CONTROL
    );
    let etag = response
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()?
        .to_string();
    let last_modified = response
        .headers()
        .get(header::LAST_MODIFIED)
        .unwrap()
        .to_str()?
        .to_string();
    let body = response.body_string().await;
    assert!(body.contains("<title>Example</title>"));

    let response = app
        .req()
        .header(header::IF_NONE_MATCH, etag.clone())
        .expect_status(StatusCode::NOT_MODIFIED)
        .get(&url)
        .await;

    assert_eq!(
        response.headers().get(header::ETAG).unwrap().to_str()?,
        etag.as_str()
    );
    assert_eq!(
        response
            .headers()
            .get(header::LAST_MODIFIED)
            .unwrap()
            .to_str()?,
        last_modified.as_str()
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        cache::CACHE_CONTROL
    );
    assert_eq!(response.body_string().await, "");

    Ok(())
}

#[test_log::test(tokio::test)]
async fn adding_a_link_invalidates_the_cached_feed() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let first_bookmark = app
        .create_bookmark(&user, "https://example.com/1", "First")
        .await;
    let list = app.create_list(&user, "Feed list").await;
    create_link(&app, &user, list.id, first_bookmark.id).await?;

    let url = list.feed_path();
    let response = app.req().get(&url).await;
    let first_etag = response
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()?
        .to_string();
    let first_body = response.body_string().await;
    assert!(first_body.contains("<title>First</title>"));

    let second_bookmark = app
        .create_bookmark(&user, "https://example.com/2", "Second")
        .await;
    create_link(&app, &user, list.id, second_bookmark.id).await?;

    let response = app.req().get(&url).await;
    let second_etag = response
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()?
        .to_string();
    let second_body = response.body_string().await;

    assert_ne!(first_etag, second_etag);
    assert!(second_body.contains("<title>First</title>"));
    assert!(second_body.contains("<title>Second</title>"));

    Ok(())
}

#[test_log::test(tokio::test)]
async fn bookmark_edits_are_served_from_cache_until_the_entry_is_removed() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let bookmark = app
        .create_bookmark(&user, "https://example.com", "Original title")
        .await;
    let list = app.create_list(&user, "Feed list").await;
    create_link(&app, &user, list.id, bookmark.id).await?;

    let url = list.feed_path();
    let body = app.req().get(&url).await.body_string().await;
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

    // Bookmark edits are invisible to the cache signal, so the cached body is
    // served until the entry is removed.
    let body = app.req().get(&url).await.body_string().await;
    assert!(body.contains("<title>Original title</title>"));
    assert!(!body.contains("<title>Updated title</title>"));

    app.state.feed_cache.remove(&list.id);

    let body = app.req().get(&url).await.body_string().await;
    assert!(body.contains("<title>Updated title</title>"));

    Ok(())
}

#[test_log::test(tokio::test)]
async fn bookmark_edits_are_served_after_the_ttl_expires() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let bookmark = app
        .create_bookmark(&user, "https://example.com", "Original title")
        .await;
    let list = app.create_list(&user, "Feed list").await;
    create_link(&app, &user, list.id, bookmark.id).await?;

    let url = list.feed_path();
    let response = app.req().get(&url).await;
    let etag = response
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()?
        .to_string();
    let body = response.body_string().await;
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

    // The edit is invisible to the signal, so the entry is still served.
    let body = app.req().get(&url).await.body_string().await;
    assert!(body.contains("<title>Original title</title>"));

    // Simulate the one-hour TTL elapsing without sleeping.
    app.state.feed_cache.expire_entry(&list.id);

    // The old ETag must no longer match the re-rendered body.
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

#[test_log::test(tokio::test)]
async fn feed_revalidates_after_removing_the_newest_link() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;

    // Create the list *before* the bookmark so that, without the fix, removing
    // the only link makes the old `feed.updated` (bookmark created_at) move
    // backwards to list.created_at. The old code then answers 304.
    let list = app.create_list(&user, "Feed list").await;
    let bookmark = app
        .create_bookmark(&user, "https://example.com", "Example")
        .await;
    let link = create_link(&app, &user, list.id, bookmark.id).await?;

    let url = list.feed_path();
    let response = app.req().get(&url).await;
    let last_modified = response
        .headers()
        .get(header::LAST_MODIFIED)
        .unwrap()
        .to_str()?
        .to_string();
    let body = response.body_string().await;
    assert!(body.contains("<title>Example</title>"));

    // HTTP dates have one-second resolution; wait so the next render produces
    // a strictly newer Last-Modified.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    let mut tx = app.tx().await;
    db::links::delete_by_id(&mut tx, link.id, user.id).await?;
    tx.commit().await?;

    let response = app
        .req()
        .header(header::IF_MODIFIED_SINCE, last_modified.clone())
        .expect_status(StatusCode::OK)
        .get(&url)
        .await;

    let new_last_modified = response
        .headers()
        .get(header::LAST_MODIFIED)
        .unwrap()
        .to_str()?
        .to_string();
    assert_ne!(new_last_modified, last_modified);

    let body = response.body_string().await;
    assert!(!body.contains("<title>Example</title>"));
    assert!(!body.is_empty());

    Ok(())
}

#[test_log::test(tokio::test)]
async fn private_lists_are_not_served_from_cache() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let list = app.create_list(&user, "Feed list").await;

    app.req().get(&list.feed_path()).await;

    let mut tx = app.tx().await;
    db::lists::set_private(&mut tx, list.id, true).await?;
    tx.commit().await?;

    app.req()
        .expect_status(StatusCode::NOT_FOUND)
        .get(&list.feed_path())
        .await;

    Ok(())
}

#[test_log::test(tokio::test)]
async fn private_linked_lists_are_removed_from_the_cached_feed() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let outer_list = app.create_list(&user, "Outer list").await;
    let inner_list = app.create_list(&user, "Inner list").await;
    create_link(&app, &user, outer_list.id, inner_list.id).await?;

    let url = outer_list.feed_path();
    let body = app.req().get(&url).await.body_string().await;
    assert!(body.contains("<title>Inner list</title>"));

    let mut tx = app.tx().await;
    db::lists::set_private(&mut tx, inner_list.id, true).await?;
    tx.commit().await?;

    // The linked list is hidden from anonymous feed requests now. Its cached
    // entry must not keep exposing it.
    let body = app.req().get(&url).await.body_string().await;
    assert!(!body.contains("<title>Inner list</title>"));

    Ok(())
}

#[test_log::test(tokio::test)]
async fn feed_of_unknown_list_returns_not_found() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;

    app.req()
        .expect_status(StatusCode::NOT_FOUND)
        .get(&format!("/lists/{}/feed.atom", Uuid::new_v4()))
        .await;

    Ok(())
}
