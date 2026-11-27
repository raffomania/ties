use axum::http::{StatusCode, header};
use sqlx::query;

use crate::{db, route_paths, tests::util::test_app::TestApp};

#[test_log::test(tokio::test)]
async fn list_metadata_counts_only_visible_bookmarks() -> anyhow::Result<()> {
    let app = TestApp::new().await;
    let list_owner = app.create_user("list_owner", "longpassword").await;
    let bookmark_owner = app.create_user("bookmark_owner", "longpassword").await;

    let list = app.create_list(&list_owner, "list").await;
    let private_bookmark = app
        .create_bookmark(&bookmark_owner, "https://example.com/private", "")
        .await;
    let public_bookmark = app
        .create_bookmark(&bookmark_owner, "https://example.com/public", "")
        .await;
    let bookmark_owner_list = app.create_list(&bookmark_owner, "public").await;
    app.create_link(&bookmark_owner, bookmark_owner_list.id, public_bookmark.id)
        .await;
    app.create_link(&list_owner, list.id, public_bookmark.id)
        .await;

    let mut tx = app.tx().await;
    query(r"insert into links (user_id, src_list_id, dest_bookmark_id) values ($1, $2, $3)")
        .bind(list_owner.id)
        .bind(list.id)
        .bind(private_bookmark.id)
        .execute(&mut *tx)
        .await?;

    assert_eq!(
        db::lists::by_id_with_metadata(&mut tx, list.id, None)
            .await?
            .metadata
            .linked_bookmark_count,
        1
    );
    assert_eq!(
        db::lists::by_id_with_metadata(&mut tx, list.id, Some(bookmark_owner.ap_user_id))
            .await?
            .metadata
            .linked_bookmark_count,
        2
    );
    assert_eq!(
        db::lists::by_id_with_metadata(&mut tx, list.id, None)
            .await?
            .metadata
            .linked_bookmark_count,
        1
    );
    assert_eq!(
        db::lists::by_id_with_metadata(&mut tx, list.id, Some(bookmark_owner.ap_user_id))
            .await?
            .metadata
            .linked_bookmark_count,
        2
    );

    let lists = db::lists::list_public_by_user(&mut tx, list_owner.ap_user_id).await?;
    let list_metadata = &lists
        .iter()
        .find(|l| l.list.id == list.id)
        .unwrap()
        .metadata;
    assert_eq!(list_metadata.linked_bookmark_count, 1);

    Ok(())
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
    let link = app.create_link(&user, list.id, bookmark.id).await;

    let url = route_paths::list_feed(list.id);
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
async fn private_lists_are_not_served_from_cache() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let list = app.create_list(&user, "Feed list").await;

    app.req().get(&route_paths::list_feed(list.id)).await;

    let mut tx = app.tx().await;
    db::lists::set_private(&mut tx, list.id, true).await?;
    tx.commit().await?;

    app.req()
        .expect_status(StatusCode::NOT_FOUND)
        .get(&route_paths::list_feed(list.id))
        .await;

    Ok(())
}

#[test_log::test(tokio::test)]
async fn private_linked_lists_are_removed_from_the_cached_feed() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    let outer_list = app.create_list(&user, "Outer list").await;
    let inner_list = app.create_list(&user, "Inner list").await;
    app.create_link(&user, outer_list.id, inner_list.id).await;

    let url = route_paths::list_feed(outer_list.id);
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
