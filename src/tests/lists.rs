use crate::tests::util::test_app::TestApp;

#[test_log::test(tokio::test)]
async fn get_create_list() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    app.create_test_user().await;
    app.login_test_user().await;

    let create_list = app.req().get("/lists/create").await.test_page().await;

    insta::assert_snapshot!(create_list.dom.find("main").htmls());

    Ok(())
}
