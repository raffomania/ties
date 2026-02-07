use pretty_assertions::assert_eq;

use crate::{
    db::{self, bookmarks::InsertBookmark},
    routes::search::SearchQuery,
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
        db::bookmarks::insert_local(
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
    }

    tx.commit().await?;

    let home = app.req().get("/").await.test_page().await;
    let search_results = home
        .fill_form(
            "form[action='/search']",
            &SearchQuery {
                q: "Rust".to_string(),
                after_bookmark_id: None,
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

    // Test partial word match - "gram" appears in "Programming" and "Guide"
    let search_results = app.req().get("/search?q=gram").await.test_page().await;
    let html = search_results.dom.htmls();
    // At least one bookmark with "Programming" or "Guide" should appear
    let has_programming = html.contains("Programming");
    let has_guide = html.contains("Guide");
    assert!(
        has_programming || has_guide,
        "Should find bookmarks containing 'gram'"
    );

    // Test special characters
    let search_results = app.req().get("/search?q=C%2B%2B").await.test_page().await;
    let html = search_results.dom.htmls();
    assert!(html.contains("C++ Programming Guide"));
    assert!(!html.contains("Python Tutorial"));

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
    let mut tx = app.tx().await;
    let mut bookmarks = Vec::new();
    for i in 1..=(page_size * 3) {
        let bookmark = db::bookmarks::insert_local(
            &mut tx,
            user.ap_user_id,
            InsertBookmark {
                url: format!("https://example.com/test{i}"),
                title: format!("Test Bookmark {i:02}"),
            },
            &app.base_url,
        )
        .await?;
        bookmarks.push(bookmark);
    }
    tx.commit().await?;

    // Sort bookmarks by ID to match the database sort order
    bookmarks.sort_by_key(|bookmark| bookmark.id);

    let assert_is_page = |page: &TestPage, n: usize| {
        // for debugging
        // for link in page.dom.find("a") {
        //     println!("- {}", link.outer_html());
        // }

        tracing::info!("Checking page {n}");

        let html = page.dom.find("main").htmls();
        assert!(html.contains(&bookmarks[page_size * n].title));
        if n > 0 {
            assert!(!html.contains(&bookmarks[(page_size * n) - 1].title));
        }
        if n < 2 {
            assert!(!html.contains(&bookmarks[page_size * (n + 1)].title));
        }
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
async fn search_requires_authentication() -> anyhow::Result<()> {
    let mut app = TestApp::new().await;

    // Try to search without logging in - should redirect to login page
    app.req()
        .expect_status(axum::http::StatusCode::SEE_OTHER)
        .get("/search?q=test")
        .await;

    Ok(())
}
