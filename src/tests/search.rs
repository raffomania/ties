use pretty_assertions::assert_eq;

use crate::{
    archive,
    db::{
        self,
        bookmarks::{InsertBookmark, UpdateBookmark},
    },
    routes::search::{self},
    tests::util::{request_builder::TestPage, test_app::TestApp},
};

#[test_log::test(tokio::test)]
async fn search_finds_bookmarks_with_various_queries() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    app.login_test_user().await;

    // Create bookmarks with different titles (6 Rust bookmarks to trigger
    // pagination since page size is 4, plus other bookmarks)
    let mut tx = app.tx().await;
    let titles = vec![
        "Learning Rust Programming",
        "Advanced Rust Patterns",
        "Rust Async Programming",
        "Rust Performance Optimization",
        "Rust Web Development",
        "Rust Macros Guide",
        // Non-Rust bookmarks to ensure search filtering works
        "Python Tutorial",
        "C++ Programming Guide",
    ];

    for title in &titles {
        let bookmark = db::bookmarks::insert_local(
            &mut tx,
            user.ap_user_id,
            InsertBookmark {
                url: format!(
                    "https://example.com/{}",
                    title.to_lowercase().replace(' ', "-")
                ),
                title: (*title).to_string(),
            },
            &app.base_url,
        )
        .await?;

        let archive = db::archives::insert_pending(&mut tx, bookmark.id).await?;
        let article = archive::make_readable(app.base_url.clone(), "<body>bodytext</body>");
        assert_eq!(article.as_ref().unwrap().text_content, "bodytext");
        db::archives::update(&mut tx, archive.id, &article).await?;
    }

    tx.commit().await?;

    let home = app.req().get("/").await.test_page().await;
    let search_results = home
        .fill_form(
            "form[action='/search']",
            &search::Query {
                q: "Rust".to_string(),
                page: None,
            },
        )
        .await
        .test_page()
        .await;

    // Test exact word match - searching for "Rust" should find all 6
    // bookmarks on first page (ordered by UUID)
    let html = search_results.dom.htmls();

    // Count how many Rust bookmarks appear in the first page
    let rust_count = titles.iter().filter(|&title| html.contains(title)).count();
    assert_eq!(
        rust_count, 6,
        "First page should show exactly 6 Rust bookmarks"
    );

    // Verify non-Rust bookmarks are not included
    assert!(!html.contains("Python Tutorial"));
    assert!(!html.contains("C++ Programming Guide"));

    // Test case insensitivity
    let search_results = app.req().get("/search?q=python").await.test_page().await;
    let html = search_results.dom.htmls();
    assert!(html.contains("Python Tutorial"));

    // Test special characters
    let search_results = app.req().get("/search?q=C%2B%2B").await.test_page().await;
    let html = search_results.dom.htmls();
    assert!(html.contains("C++ Programming Guide"));
    assert!(!html.contains("Python Tutorial"));

    // Test article body
    let search_results = app.req().get("/search?q=bodytext").await.test_page().await;
    let html = search_results.dom.htmls();

    let results_count = titles.iter().filter(|&title| html.contains(title)).count();
    assert_eq!(
        results_count,
        titles.len(),
        "Searching for bodytext should show all bookmarks",
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn search_only_returns_users_own_bookmarks() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user1 = app.create_test_user().await;
    let user2 = app.create_user("otheruser", "otherpassword").await;

    // Create bookmarks for both users with similar titles
    let mut tx = app.tx().await;
    let bookmark_1 = db::bookmarks::insert_local(
        &mut tx,
        user1.ap_user_id,
        InsertBookmark {
            url: "https://example.com/user1".to_string(),
            title: "My Rust Tutorial".to_string(),
        },
        &app.base_url,
    )
    .await?;
    let bookmark_2 = db::bookmarks::insert_local(
        &mut tx,
        user2.ap_user_id,
        InsertBookmark {
            url: "https://example.com/user2".to_string(),
            title: "Other User's Rust Guide".to_string(),
        },
        &app.base_url,
    )
    .await?;
    tx.commit().await?;

    let query = "/search?q=Rust";
    // Login as user1 and search
    app.login_test_user().await;
    let search_results = app.req().get(query).await.test_page().await;

    let html = search_results.dom.htmls();
    assert!(html.contains(&bookmark_1.id.to_string()));
    assert!(!html.contains(&bookmark_2.id.to_string()));

    // verify that the same query matches the other bookmark as well
    app.login_user(&user2.username, "otherpassword").await;
    let search_results = app.req().get(query).await.test_page().await;
    let html = search_results.dom.htmls();
    tracing::debug!("{}", search_results.dom.find("main").html());
    assert!(!html.contains(&bookmark_1.id.to_string()));
    assert!(html.contains(&bookmark_2.id.to_string()));

    Ok(())
}

#[test_log::test(tokio::test)]
async fn search_pagination_navigation() -> anyhow::Result<()> {
    // TODO update this test to use the pagination links provided in the html,
    // instead of generating the URLs inline here
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    app.login_test_user().await;

    let page_size = 50;
    // Create enough bookmarks to span multiple pages
    let mut bookmarks = Vec::new();
    for i in 1..=(page_size * 3) {
        let bookmark = app
            .create_bookmark(&user, &format!("https://example.com/test{i}"), "test")
            .await;
        bookmarks.push(bookmark);
    }

    let assert_is_page = |page: &TestPage, n: usize| {
        // for debugging
        for link in page.dom.find("a") {
            println!("- {}", link.outer_html());
        }

        tracing::info!("Checking page {n}");

        let html = page.dom.find("main").htmls();
        if n < 2 {
            assert!(html.contains("Next page"));
        } else {
            assert!(!html.contains("Next page"));
        }
        if n > 0 {
            assert!(html.contains("Previous page"));
        } else {
            assert!(!html.contains("Previous page"));
        }
    };

    // Test first page - should show first 4 bookmarks sorted by ID
    let first_page = app.req().get("/search?q=Test").await.test_page().await;
    assert_is_page(&first_page, 0);

    // Test second page (forward pagination)
    let second_page = first_page.visit_link("Next page").await;
    assert_is_page(&second_page, 1);

    let third_page = second_page.visit_link("Next page").await;
    assert_is_page(&third_page, 2);

    // Test backward pagination - go back to second page
    let back_to_second = third_page.visit_link("Previous page").await;
    assert_is_page(&back_to_second, 1);

    Ok(())
}

#[test_log::test(tokio::test)]
async fn search_preserves_query_in_pagination() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    app.create_test_user().await;
    app.login_test_user().await;

    let search_results = app.req().get("/search?q=Rust").await.test_page().await;

    // Check that the search query is preserved in the pagination form
    assert!(search_results.dom.html().contains(r#"value="Rust""#));

    Ok(())
}

#[test_log::test(tokio::test)]
async fn update_bookmark_updates_search_index() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;
    let user = app.create_test_user().await;
    app.login_test_user().await;

    let mut tx = app.tx().await;
    let bookmark = db::bookmarks::insert_local(
        &mut tx,
        user.ap_user_id,
        InsertBookmark {
            url: "https://example.com/renamed-bookmark".to_string(),
            title: "Original Title".to_string(),
        },
        &app.base_url,
    )
    .await?;
    tx.commit().await?;

    // Verify the bookmark is found by its original title
    let search_results = app.req().get("/search?q=Original").await.test_page().await;
    let html = search_results.dom.htmls();
    assert!(
        html.contains("Original Title"),
        "should find bookmark by original title before rename"
    );

    // Rename the bookmark
    let mut tx = app.tx().await;
    db::bookmarks::update_local(
        &mut tx,
        bookmark.id,
        UpdateBookmark {
            title: "Updated Title".to_string(),
        },
        bookmark.ap_user_id,
    )
    .await?;
    tx.commit().await?;

    // Verify the bookmark is found by the new title
    let search_results = app.req().get("/search?q=Updated").await.test_page().await;
    let html = search_results.dom.htmls();
    assert!(
        html.contains("Updated Title"),
        "should find bookmark by new title after rename"
    );

    // Verify the old title no longer appears in search
    let search_results = app.req().get("/search?q=Original").await.test_page().await;
    let html = search_results.dom.htmls();
    assert!(
        !html.contains("Original Title"),
        "should not find bookmark by old title after rename"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn search_requires_authentication() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;

    // Try to search without logging in - should redirect to login page
    app.req()
        .expect_status(axum::http::StatusCode::SEE_OTHER)
        .get("/search?q=test")
        .await;

    Ok(())
}
