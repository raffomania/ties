use axum::http::StatusCode;

use crate::tests::util::test_app::TestApp;

#[test_log::test(tokio::test)]
async fn index() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;

    app.req()
        .expect_status(StatusCode::SEE_OTHER)
        .get("/")
        .await;

    app.create_test_user().await;
    app.login_test_user().await;
    // Check that we can access the index when logged in
    app.req().get("/").await.test_page().await;

    // no snapshot here, since the bookmarklet contains the base url which contains
    // a new random port every time the test runs insta::assert_snapshot!(index.
    // dom.htmls());

    Ok(())
}
