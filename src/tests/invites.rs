use axum::http::StatusCode;
use serde::Serialize;

use crate::{db, forms, tests::util::test_app::TestApp};

#[derive(Serialize, Default)]
struct EmptyForm;

#[test_log::test(tokio::test)]
async fn create_invite_and_use_it() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let inviting_user = app.create_test_user().await;
    app.login_test_user().await;

    // Create an invite
    let create_response = app
        .req()
        .expect_status(StatusCode::OK)
        .post("/invites/create", &EmptyForm)
        .await;

    let dom = create_response.dom().await;
    let invite_url = dom
        .find("pre")
        .into_iter()
        .next()
        .expect("Invite URL not found in response")
        .text_content();
    let invite_url = url::Url::parse(&invite_url)?;
    let token = invite_url
        .path()
        .strip_prefix("/invites/")
        .expect("Invite URL path should start with /invites/")
        .to_string();

    assert!(!token.is_empty(), "Invite token should not be empty");

    // Accept the invite with a new user
    let accept_response = app
        .req()
        .expect_status(StatusCode::SEE_OTHER)
        .post(
            &invite_url.path(),
            &forms::users::CreateUser {
                username: "invited_user".to_string(),
                password: "securepassword123".to_string(),
            },
        )
        .await;

    // Verify redirect to /
    assert_eq!(accept_response.headers().get("location").unwrap(), "/");

    // Verify the new user was created in the database
    let mut tx = app.pool.begin().await?;
    let invited_user = db::users::by_username(&mut tx, "invited_user").await?;
    assert_eq!(invited_user.username, "invited_user");
    assert_eq!(invited_user.invited_by, Some(inviting_user.id));

    // Verify the invite was deleted after use
    let invite = db::invites::by_token(&mut tx, &token).await?;
    assert!(
        invite.is_none(),
        "Invite should be deleted after account creation"
    );

    // Verify the invited user can log in using the session cookie from the invite
    // acceptance
    let invite_cookie = accept_response
        .cookie(crate::session::COOKIE_NAME)
        .expect("accept response should set a session cookie");

    app.req()
        .header(
            axum::http::header::COOKIE,
            invite_cookie.parse::<axum::http::HeaderValue>()?,
        )
        .get("/")
        .await;

    Ok(())
}
