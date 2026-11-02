use activitypub_federation::fetch::object_id::ObjectId;
use axum::http::StatusCode;
use url::Url;
use uuid::Uuid;

use crate::{
    db::{self, bookmarks::InsertBookmark},
    forms::{self, links::CreateLink, lists::CreateList},
    tests::util::{dom::assert_form_matches, test_app::TestApp},
};

#[test_log::test(tokio::test)]
async fn get_unsorted_bookmarks() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    app.create_test_user().await;
    app.login_test_user().await;

    let unsorted_bookmarks = app.req().get("/bookmarks/unsorted").await.test_page().await;

    insta::assert_snapshot!(unsorted_bookmarks.dom.find("main").htmls());

    Ok(())
}

#[test_log::test(tokio::test)]
async fn get_create_bookmark() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    app.login_test_user().await;
    app.create_list(&user, "recent test list").await;

    let form_page = app.req().get("/bookmarks/create").await.test_page().await;

    assert_form_matches(
        &form_page.dom,
        &forms::bookmarks::CreateBookmark {
            url: "https://ties.pub".to_string(),
        },
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn is_bookmark_public() -> anyhow::Result<()> {
    let app = TestApp::new().await;
    let user = app.create_test_user().await;

    let mut tx = app.tx().await;
    let bookmark = db::bookmarks::insert_local(
        &mut tx,
        user.ap_user_id,
        InsertBookmark {
            url: String::new(),
            title: None,
        },
        &app.base_url,
    )
    .await?;

    assert!(!db::bookmarks::is_public(&mut tx, bookmark.id).await?);

    let private_list = db::lists::insert(
        &mut tx,
        user.ap_user_id,
        CreateList {
            title: String::new(),
            content: None,
            private: true,
        },
    )
    .await?;
    db::links::insert(
        &mut tx,
        user.id,
        user.ap_user_id,
        CreateLink {
            src: private_list.id,
            dest: bookmark.id,
        },
    )
    .await?;

    assert!(!db::bookmarks::is_public(&mut tx, bookmark.id).await?);

    let public_list = db::lists::insert(
        &mut tx,
        user.ap_user_id,
        CreateList {
            title: String::new(),
            content: None,
            private: false,
        },
    )
    .await?;
    db::links::insert(
        &mut tx,
        user.id,
        user.ap_user_id,
        CreateLink {
            src: public_list.id,
            dest: bookmark.id,
        },
    )
    .await?;

    assert!(db::bookmarks::is_public(&mut tx, bookmark.id).await?);

    Ok(())
}

#[test_log::test(tokio::test)]
async fn delete() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let owner = app.create_test_user().await;
    app.login_test_user().await;

    let bookmark = app
        .create_bookmark(&owner, "https://example.com", "Test")
        .await;

    app.req()
        .delete(&format!("/bookmarks/{}", bookmark.id))
        .await;

    Ok(())
}

#[test_log::test(tokio::test)]
async fn only_owner_can_delete_bookmark() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let owner = app.create_user("owner", "longpassword").await;
    let _other = app.create_user("other", "longpassword").await;

    let bookmark = app
        .create_bookmark(&owner, "https://example.com", "Test")
        .await;

    app.login_user("other", "longpassword").await;
    app.req()
        .expect_status(StatusCode::NOT_FOUND)
        .delete(&format!("/bookmarks/{}", bookmark.id))
        .await;

    app.login_user("owner", "longpassword").await;
    app.req()
        .expect_status(StatusCode::OK)
        .delete(&format!("/bookmarks/{}", bookmark.id))
        .await;

    app.req()
        .expect_status(StatusCode::NOT_FOUND)
        .delete(&format!("/bookmarks/{}", Uuid::new_v4()))
        .await;

    Ok(())
}

/// This test documents a known bug on purpose: it checks what the code does
/// today, even though that is wrong, so that the test starts failing as soon as
/// the bug is fixed. The failure message tells whoever fixes the bug what to
/// check instead.
///
/// The bug is in `db::bookmarks::upsert_remote`, the function that saves a
/// bookmark we received from another ties server. Saving a bookmark we have
/// not seen before works fine. But when a bookmark we already know has changed
/// on the other server, the update writes to the wrong columns: the id of the
/// newly created row ends up in the column that stores who owns the bookmark.
/// No user with that id exists, so the database rejects the update and nothing
/// changes.
///
/// This also matters for the rules in doc/permissions.md, which say that only
/// the owner of a bookmark may change it. Saving must never change who owns a
/// bookmark, and today it would if the database did not reject it.
///
/// When the bug is fixed, this test fails on purpose. Then replace its checks
/// with checks for the correct behavior: the bookmark keeps its owner and gets
/// the new URL and title.
#[test_log::test(tokio::test)]
async fn upsert_remote_update_bug_is_still_present() -> anyhow::Result<()> {
    let app = TestApp::new().await;
    let user = app.create_test_user().await;

    let ap_id = ObjectId::from(Url::parse("https://remote.example/ap/bookmark/1")?);

    // Save the remote bookmark for the first time. There is nothing to update
    // yet, so this works fine.
    let mut tx = app.tx().await;
    let first = db::bookmarks::upsert_remote(
        &mut tx,
        user.ap_user_id,
        &ap_id,
        InsertBookmark {
            url: "https://old.example".to_string(),
            title: Some("Old Title".to_string()),
        },
    )
    .await?;
    tx.commit().await?;
    assert_eq!(first.url, "https://old.example");

    // Save the same remote bookmark again, now with a changed URL and title.
    // This takes the "update the saved bookmark" path, which is where the bug
    // is.
    let mut tx = app.tx().await;
    let result = db::bookmarks::upsert_remote(
        &mut tx,
        user.ap_user_id,
        &ap_id,
        InsertBookmark {
            url: "https://new.example".to_string(),
            title: Some("New Title".to_string()),
        },
    )
    .await;

    let err = result.expect_err(
        "Saving a known remote bookmark again unexpectedly worked: the bug in \
         the save query in src/db/bookmarks.rs seems to be fixed. Update this \
         test to check the correct behavior instead: the saved bookmark has \
         the URL \"https://new.example\", the title \"New Title\", and the same \
         owner as before.",
    );

    // Today the update fails because it puts the id of the new row into the
    // owner column, and no user with that id exists.
    let msg = format!("{err:?}");
    assert!(
        msg.contains("bookmarks_ap_user_id_fkey"),
        "Saving failed, but not with the error we expect from this bug: {msg:?}"
    );

    Ok(())
}
