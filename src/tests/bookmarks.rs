use activitypub_federation::fetch::object_id::ObjectId;
use axum::http::StatusCode;
use url::Url;
use uuid::Uuid;

use crate::{
    db::{self, bookmarks::InsertBookmark, follows, list_follows},
    forms::{self, bookmarks::ConnectToList, links::CreateLink, lists::CreateList},
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

/// Check that only links by a bookmark's owner will make that bookmark public.
#[test_log::test(tokio::test)]
async fn public_list_owned_by_someone_else_does_not_make_bookmark_public() -> anyhow::Result<()> {
    let app = TestApp::new().await;
    let owner = app.create_user("owner", "longpassword").await;
    let other = app.create_user("other", "longpassword").await;
    let bookmark = app.create_bookmark(&owner, "", "").await;

    let mut tx = app.tx().await;

    // Create public list owned by "other" and link to the bookmark
    let public_list = db::lists::insert(
        &mut tx,
        other.ap_user_id,
        CreateList {
            title: String::new(),
            content: None,
            private: false,
        },
    )
    .await?;
    // use inline query since we forbid creating such links normally
    sqlx::query(
        r#"
        insert into links (user_id, src_list_id, dest_bookmark_id)
        values ($1, $2, $3)
        "#,
    )
    .bind(other.id)
    .bind(public_list.id)
    .bind(bookmark.id)
    .execute(&mut *tx)
    .await?;

    // Check that bookmark is not public according to our queries
    assert!(!db::bookmarks::is_public(&mut tx, bookmark.id).await?);
    assert!(db::bookmarks::is_public_or_owner(&mut tx, owner.ap_user_id, bookmark.id).await?);
    assert!(!db::bookmarks::is_public_or_owner(&mut tx, other.ap_user_id, bookmark.id).await?);

    Ok(())
}

/// Check that the edit form accurately displays bookmark visibility.
#[test_log::test(tokio::test)]
async fn edit_bookmark_shows_database_visibility() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    app.login_test_user().await;

    let bookmark = app
        .create_bookmark(&user, "https://example.com", "Test")
        .await;

    // Check that it's private by default
    let page = app
        .req()
        .get(&format!("/bookmarks/{}/edit", bookmark.id))
        .await
        .test_page()
        .await;
    let visibility = page
        .dom
        .find("dl dd")
        .into_iter()
        .last()
        .unwrap()
        .text_content();
    assert_eq!(visibility, "Private");

    // Make it public and check that the form reflects this
    let public_list = app.create_list(&user, "public").await;
    let mut tx = app.tx().await;
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
    tx.commit().await?;

    let page = app
        .req()
        .get(&format!("/bookmarks/{}/edit", bookmark.id))
        .await
        .test_page()
        .await;
    let visibility = page
        .dom
        .find("dl dd")
        .into_iter()
        .last()
        .unwrap()
        .text_content();
    assert_eq!(visibility, "Public");

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

/// Saving a bookmark we received from another ties server must work both the
/// first time we see the bookmark and later, when the bookmark has changed on
/// the other server and we save it again. The second save must apply the new
/// URL and title, and it must never change who owns the bookmark.
#[test_log::test(tokio::test)]
async fn upsert_remote_updates_a_known_bookmark_but_keeps_its_owner() -> anyhow::Result<()> {
    let app = TestApp::new().await;
    let user = app.create_test_user().await;
    let other_user = app.create_user("otheruser", "longpassword").await;

    let ap_id = ObjectId::from(Url::parse("https://remote.example/ap/bookmark/1")?);

    // Save the remote bookmark for the first time.
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
    assert_eq!(first.title.as_deref(), Some("Old Title"));
    assert_eq!(first.ap_user_id, user.ap_user_id);

    // Save the same remote bookmark again after it changed on the other
    // server: the saved bookmark gets the new URL and title.
    let mut tx = app.tx().await;
    let updated = db::bookmarks::upsert_remote(
        &mut tx,
        user.ap_user_id,
        &ap_id,
        InsertBookmark {
            url: "https://new.example".to_string(),
            title: Some("New Title".to_string()),
        },
    )
    .await?;
    tx.commit().await?;
    assert_eq!(updated.url, "https://new.example");
    assert_eq!(updated.title.as_deref(), Some("New Title"));
    assert_eq!(updated.id, first.id);
    assert_eq!(updated.ap_user_id, first.ap_user_id);

    // Even a save that names another user as the author must not hand the
    // bookmark over to that user: only the owner of a bookmark may change it
    let mut tx = app.tx().await;
    let after_other_author = db::bookmarks::upsert_remote(
        &mut tx,
        other_user.ap_user_id,
        &ap_id,
        InsertBookmark {
            url: "https://other.example".to_string(),
            title: Some("Other Title".to_string()),
        },
    )
    .await?;
    tx.commit().await?;
    assert_eq!(after_other_author.id, first.id);
    assert_eq!(after_other_author.ap_user_id, first.ap_user_id);

    Ok(())
}
#[test_log::test(tokio::test)]
async fn connecting_bookmark_to_public_list_inserts_follows() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let owner = app.create_test_user().await;
    app.login_test_user().await;

    let followed = app.create_user("followed", "longpassword").await;

    let bookmark = app
        .create_bookmark(
            &owner,
            &format!("{}/user/followed", app.base_url),
            "Followed User Profile",
        )
        .await;

    let public_list = app.create_list(&owner, "my public list").await;

    app.req()
        .post(
            &format!("/bookmarks/{}/connect", bookmark.id),
            &ConnectToList {
                connect_list_id: Some(public_list.id),
            },
        )
        .await;

    let mut tx = app.tx().await;

    let follow =
        follows::read_by_follower_and_following(&mut tx, owner.ap_user_id, followed.ap_user_id)
            .await?;

    list_follows::read(&mut tx, public_list.id, bookmark.id, follow.id).await?;

    Ok(())
}
