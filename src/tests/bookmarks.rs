use axum::http::StatusCode;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    db::{self, bookmarks::InsertBookmark},
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

    let follow_row = sqlx::query(
        r"select id from follows
           where follower_id = $1 and following_id = $2",
    )
    .bind(owner.ap_user_id)
    .bind(followed.ap_user_id)
    .fetch_one(&mut *tx)
    .await?;
    let follow_id: uuid::Uuid = follow_row.get("id");

    sqlx::query(
        r"select id from list_bookmarked_user_follows
           where list_id = $1 and bookmark_id = $2 and follow_id = $3",
    )
    .bind(public_list.id)
    .bind(bookmark.id)
    .bind(follow_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}
