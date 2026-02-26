use crate::{
    db::{self, bookmarks::InsertBookmark},
    forms::{links::CreateLink, lists::CreateList},
    tests::util::test_app::TestApp,
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
async fn is_bookmark_public() -> anyhow::Result<()> {
    let app = TestApp::new().await;
    let user = app.create_test_user().await;

    let mut tx = app.tx().await;
    let bookmark = db::bookmarks::insert_local(
        &mut tx,
        user.ap_user_id,
        InsertBookmark {
            url: String::new(),
            title: String::new(),
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
        CreateLink {
            src: public_list.id,
            dest: bookmark.id,
        },
    )
    .await?;

    assert!(db::bookmarks::is_public(&mut tx, bookmark.id).await?);

    Ok(())
}
