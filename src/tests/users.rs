use axum::http::{StatusCode, header};

use crate::{
    db::{self, ap_users},
    federation::webfinger,
    forms::users::{CreateOidcUser, Credentials, Login},
    tests::util::test_app::TestApp,
};

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
    app_b.serve().await;
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
