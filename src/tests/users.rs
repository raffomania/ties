use axum::http::{StatusCode, header};
use serde::Serialize;

use crate::{
    atom::cache,
    db::{self, ap_users, bookmarks::UpdateBookmark},
    federation::webfinger,
    forms::users::{CreateOidcUser, Credentials, Login},
    session,
    tests::util::test_app::TestApp,
};

#[derive(Serialize, Default)]
struct EmptyForm;

#[test_log::test(tokio::test)]
async fn can_login() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    app.create_user("test", "testpassword").await;

    let login_page = app.req().get("/login").await.test_page().await;
    insta::assert_snapshot!(login_page.dom.find("form").outer_htmls());

    let input = Login {
        credentials: Credentials {
            username: "test".to_string(),
            password: "testpassword".to_string(),
        },
        previous_uri: None,
    };

    let login_response = login_page
        .expect_status(StatusCode::SEE_OTHER)
        .fill_form("form", &input)
        .await;

    let cookie = login_response.headers().get("Set-Cookie").unwrap();
    assert!(!cookie.is_empty());
    let cookie = cookie.to_str()?.split_once(';').unwrap().0;

    app.req().header(header::COOKIE, cookie).get("/").await;

    Ok(())
}

#[test_log::test(tokio::test)]
async fn demo_login_unavailable_without_demo_mode() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;

    // The demo login must not create accounts when demo mode is disabled.
    let response = app
        .req()
        .expect_status(StatusCode::NOT_FOUND)
        .post("/login_demo", &EmptyForm)
        .await;

    // No account should have been logged in either.
    assert!(response.cookie(session::COOKIE_NAME).is_none());

    Ok(())
}

#[test_log::test(tokio::test)]
async fn demo_login_creates_account_in_demo_mode() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    app.set_demo_mode(true);

    let response = app
        .req()
        .expect_status(StatusCode::SEE_OTHER)
        .post("/login_demo", &EmptyForm)
        .await;

    assert_eq!(response.headers().get("location").unwrap(), "/");
    let cookie = response
        .cookie(session::COOKIE_NAME)
        .expect("demo login should set a session cookie");
    assert!(!cookie.is_empty());

    // The temporary account should be able to access pages that require a
    // login.
    app.logged_in_cookie = Some(cookie);
    app.req().get("/lists/create").await;

    Ok(())
}

#[test_log::test(tokio::test)]
async fn insert_oidc_creates_ap_user() -> anyhow::Result<()> {
    let app = TestApp::new().await;
    let mut tx = app.pool.begin().await?;

    let create_oidc_user = CreateOidcUser {
        oidc_id: "test_oidc_id".to_string(),
        email: "test@example.com".to_string(),
        username: "test_oidc_user".to_string(),
    };

    // Insert OIDC user
    let user = crate::db::users::insert_oidc(&mut tx, create_oidc_user, &app.base_url).await?;

    // Verify that the user has a valid ap_user_id
    let ap_user = ap_users::read_by_id(&mut tx, user.ap_user_id).await?;

    // Verify AP user properties
    assert_eq!(ap_user.username, "test_oidc_user");

    Ok(())
}

#[test_log::test(tokio::test)]
async fn profile() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;

    let user = app.create_test_user().await;
    app.login_test_user().await;
    // Check that we can access the logged in user's profile page
    let profile = app
        .req()
        .get(&format!("/user/{}", user.username))
        .await
        .test_page()
        .await;

    let lists_header = profile
        .dom
        .find("section > p > span")
        .into_iter()
        .find(|el| el.text_content().contains("public lists"))
        .unwrap()
        .text_content();

    let mut tx = app.pool.begin().await?;
    let public_lists = db::lists::list_public_by_user(&mut tx, user.ap_user_id)
        .await?
        .len();

    assert_eq!(lists_header, format!("{public_lists} public lists"));

    Ok(())
}

#[test_log::test(tokio::test)]
async fn user_feed_only_contains_publicly_linked_bookmarks() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;

    let public_bookmark = app
        .create_bookmark(&user, "https://example.com/public", "Public bookmark")
        .await;
    let public_list = app.create_list(&user, "Public list").await;
    app.create_link(&user, public_list.id, public_bookmark.id)
        .await;

    let private_bookmark = app
        .create_bookmark(&user, "https://example.com/private", "Private bookmark")
        .await;
    let private_list = app.create_list(&user, "Private list").await;
    let mut tx = app.tx().await;
    db::lists::set_private(&mut tx, private_list.id, true).await?;
    tx.commit().await?;

    app.create_link(&user, private_list.id, private_bookmark.id)
        .await;

    app.create_bookmark(&user, "https://example.com/unsorted", "Unsorted bookmark")
        .await;

    let body = app.req().get(&user.feed_path()).await.body_string().await;

    assert!(body.contains("<title>Public bookmark</title>"));
    assert!(!body.contains("<title>Private bookmark</title>"));
    assert!(!body.contains("https://example.com/private"));
    assert!(!body.contains("<title>Unsorted bookmark</title>"));
    assert!(!body.contains("https://example.com/unsorted"));

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
    app.create_link(&user, list.id, bookmark.id).await;

    let url = user.feed_path();
    let response = app.req().get(&url).await;

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
    assert!(body.contains("<title>Example</title>"));

    let response = app
        .req()
        .header(header::IF_NONE_MATCH, etag.clone())
        .expect_status(StatusCode::NOT_MODIFIED)
        .get(&url)
        .await;

    assert_eq!(response.header_string(header::ETAG), etag);
    assert_eq!(response.header_string(header::LAST_MODIFIED), last_modified);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        cache::CACHE_CONTROL
    );
    assert_eq!(response.body_string().await, "");

    Ok(())
}

#[test_log::test(tokio::test)]
async fn adding_a_bookmark_invalidates_the_cached_feed() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let first_bookmark = app
        .create_bookmark(&user, "https://example.com/1", "First")
        .await;
    let list = app.create_list(&user, "Feed list").await;
    app.create_link(&user, list.id, first_bookmark.id).await;

    let url = user.feed_path();
    let response = app.req().get(&url).await;
    let first_etag = response.header_string(header::ETAG);
    let first_body = response.body_string().await;
    assert!(first_body.contains("<title>First</title>"));

    let second_bookmark = app
        .create_bookmark(&user, "https://example.com/2", "Second")
        .await;
    app.create_link(&user, list.id, second_bookmark.id).await;

    let response = app.req().get(&url).await;
    let second_etag = response.header_string(header::ETAG);
    let second_body = response.body_string().await;

    assert_ne!(first_etag, second_etag);
    assert!(second_body.contains("<title>First</title>"));
    assert!(second_body.contains("<title>Second</title>"));

    Ok(())
}

#[test_log::test(tokio::test)]
async fn bookmark_edits_are_served_from_cache_until_the_ttl_expires() -> anyhow::Result<()> {
    let mut app = TestApp::with_feed_cache_ttl(std::time::Duration::from_secs(2)).await;
    let user = app.create_test_user().await;
    let bookmark = app
        .create_bookmark(&user, "https://example.com", "Original title")
        .await;
    let list = app.create_list(&user, "Feed list").await;
    app.create_link(&user, list.id, bookmark.id).await;

    let url = user.feed_path();
    let response = app.req().get(&url).await;
    let etag = response.header_string(header::ETAG);
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

    let body = app.req().get(&url).await.body_string().await;
    assert!(body.contains("<title>Original title</title>"));
    assert!(!body.contains("<title>Updated title</title>"));

    tokio::time::sleep(std::time::Duration::from_millis(2200)).await;

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
    // TODO-DOC: This test fails because db::users::feed_cache_signal doesn't
    // track links, so the stale cached feed is still served after unlinking a
    // bookmark.
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;

    let list = app.create_list(&user, "Feed list").await;
    let bookmark = app
        .create_bookmark(&user, "https://example.com", "Example")
        .await;
    let link = app.create_link(&user, list.id, bookmark.id).await;

    let url = user.feed_path();
    let response = app.req().get(&url).await;
    let last_modified = response.header_string(header::LAST_MODIFIED);
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

    let new_last_modified = response.header_string(header::LAST_MODIFIED);
    assert_ne!(new_last_modified, last_modified);

    let body = response.body_string().await;
    assert!(!body.contains("<title>Example</title>"));
    assert!(!body.is_empty());

    Ok(())
}

#[test_log::test(tokio::test)]
async fn private_lists_are_removed_from_the_cached_feed() -> anyhow::Result<()> {
    // TODO-DOC: This test fails because db::users::feed_cache_signal doesn't
    // track list privacy, so the cached feed keeps exposing private content
    // until the cache TTL expires.
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let bookmark = app
        .create_bookmark(&user, "https://example.com", "Example")
        .await;
    let list = app.create_list(&user, "Feed list").await;
    app.create_link(&user, list.id, bookmark.id).await;

    let url = user.feed_path();
    let body = app.req().get(&url).await.body_string().await;
    assert!(body.contains("<title>Feed list</title>"));
    assert!(body.contains("<title>Example</title>"));

    let mut tx = app.tx().await;
    db::lists::set_private(&mut tx, list.id, true).await?;
    tx.commit().await?;

    let body = app.req().get(&url).await.body_string().await;
    assert!(!body.contains("<title>Feed list</title>"));
    assert!(!body.contains("<title>Example</title>"));

    Ok(())
}

#[test_log::test(tokio::test)]
async fn feed_of_unknown_user_returns_not_found() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;

    app.req()
        .expect_status(StatusCode::NOT_FOUND)
        .get("/user/nonexistent/feed.atom")
        .await;

    Ok(())
}

#[test_log::test(tokio::test)]
async fn feed_renders_for_user_with_lists_but_no_bookmarks() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    app.create_list(&user, "Public list").await;

    let body = app.req().get(&user.feed_path()).await.body_string().await;

    assert!(body.contains("<title>Public list</title>"));

    Ok(())
}

#[test_log::test(tokio::test)]
async fn feed_renders_for_user_with_bookmarks_but_no_lists() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    app.create_bookmark(&user, "https://example.com", "Example")
        .await;

    let response = app.req().get(&user.feed_path()).await;

    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/atom+xml; charset=utf-8"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn feed_renders_for_user_without_bookmarks_or_lists() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;

    let response = app.req().get(&user.feed_path()).await;

    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/atom+xml; charset=utf-8"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn profile_remote_user() -> anyhow::Result<()> {
    let mut app_a = TestApp::new().await;
    app_a.create_test_user().await;

    let app_b = TestApp::new().await;
    let user_to_show = app_b.create_user("testb", "testpassword").await;
    let user_to_show_ap_user = db::ap_users::read_by_username(
        &mut app_b.pool.begin().await?,
        webfinger::Resource::from_name_and_url(user_to_show.username.clone(), &app_b.base_url)?,
    )
    .await?;

    // Fetch the user from B and store it in A's db
    app_b.serve();
    let ap_cx_a = app_a.state.federation_config.to_request_data();
    user_to_show_ap_user.ap_id.dereference(&ap_cx_a).await?;

    // Check that A can display the profile for the remote user
    app_a.login_test_user().await;
    app_a
        .req()
        .get(&format!(
            "/user/{}@{}:{}",
            user_to_show.username,
            app_b.base_url.domain().unwrap(),
            app_b.base_url.port().unwrap()
        ))
        .await
        .test_page()
        .await;

    Ok(())
}

#[test_log::test(tokio::test)]
async fn feed_of_remote_user_returns_not_found() -> anyhow::Result<()> {
    let mut app_a = TestApp::new().await;
    app_a.create_test_user().await;

    let app_b = TestApp::new().await;
    let remote_user = app_b.create_user("testb", "testpassword").await;
    let remote_ap_user = db::ap_users::read_by_username(
        &mut app_b.pool.begin().await?,
        webfinger::Resource::from_name_and_url(remote_user.username.clone(), &app_b.base_url)?,
    )
    .await?;

    // Fetch the user from B and store it in A's db
    app_b.serve();
    let ap_cx_a = app_a.state.federation_config.to_request_data();
    remote_ap_user.ap_id.dereference(&ap_cx_a).await?;

    // Feeds are only supported for local users
    app_a
        .req()
        .expect_status(StatusCode::NOT_FOUND)
        .get(&format!(
            "/user/{}@{}:{}/feed.atom",
            remote_user.username,
            app_b.base_url.domain().unwrap(),
            app_b.base_url.port().unwrap()
        ))
        .await;

    Ok(())
}
