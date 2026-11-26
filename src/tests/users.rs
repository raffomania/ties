use axum::http::{StatusCode, header};
use serde::Serialize;

use crate::{
    db::{self, ap_users},
    federation::webfinger,
    forms::{
        links::CreateLink,
        users::{CreateOidcUser, Credentials, Login},
    },
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
async fn profile_counts_only_public_bookmarks_in_public_lists() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let profile_owner = app.create_user("profile_owner", "longpassword").await;
    let bookmark_owner = app.create_user("bookmark_owner", "longpassword").await;

    let public_list = app.create_list(&profile_owner, "public").await;
    let private_bookmark = app
        .create_bookmark(&bookmark_owner, "https://example.com/private", "")
        .await;
    let public_bookmark = app
        .create_bookmark(&bookmark_owner, "https://example.com/public", "")
        .await;
    let bookmark_owner_list = app.create_list(&bookmark_owner, "public").await;

    let mut tx = app.tx().await;
    db::links::insert(
        &mut tx,
        bookmark_owner.id,
        bookmark_owner.ap_user_id,
        CreateLink {
            src: bookmark_owner_list.id,
            dest: public_bookmark.id,
        },
    )
    .await?;
    db::links::insert(
        &mut tx,
        profile_owner.id,
        profile_owner.ap_user_id,
        CreateLink {
            src: public_list.id,
            dest: public_bookmark.id,
        },
    )
    .await?;
    sqlx::query(r"insert into links (user_id, src_list_id, dest_bookmark_id) values ($1, $2, $3)")
        .bind(profile_owner.id)
        .bind(public_list.id)
        .bind(private_bookmark.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let page = app
        .req()
        .get(&format!("/user/{}", profile_owner.username))
        .await
        .test_page()
        .await;
    let summary = page
        .dom
        .find("section > p")
        .into_iter()
        .find(|p| p.text_content().contains("containing"))
        .unwrap()
        .text_content();
    assert_eq!(summary, "1 public list containing 1 bookmark");

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
