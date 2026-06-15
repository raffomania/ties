use url::Url;

use crate::{archive, db, tests::util::test_app::TestApp};

#[test]
fn htmx_attributes_stripped() {
    let malicious_html = r#"<html><head><title>Article</title></head><body>
        <article>
            <h1>Real Article Title</h1>
            <p>Enough content to pass readability threshold and be extracted as the article body.</p>
            <p>More text content here to ensure the article is long enough for extraction.</p>
            <p>Continuing with more content to meet the minimum length requirements.</p>
            <p hx-get="/bookmarks/some-id" hx-target="main" hx-swap="innerHTML" data-hx-post="/">
            malicious element
            </p>
        </article>
    </body></html>"#;

    let base_url = Url::parse("https://evil.example.com").unwrap();
    let article = archive::make_readable(base_url, malicious_html).unwrap();

    assert!(article.content.contains("malicious element"));

    assert!(!article.content.contains("hx-get"),);
    assert!(!article.content.contains("data-hx-post"),);
    assert!(!article.content.contains("hx-target"),);
    assert!(!article.content.contains("hx-swap"),);
}
#[test_log::test(tokio::test)]
#[ignore = "Test depends on an external resource and should only be run manually."]
async fn flaky_test_get_website() -> anyhow::Result<()> {
    let text = archive::fetch_url_as_text("https://rafa.ee").await?;
    assert!(!text.is_empty());

    let image_err = archive::fetch_url_as_text("https://www.rafa.ee/portrait.jpg").await;
    image_err.unwrap_err();

    let blocked_ip_err = archive::fetch_url_as_text("https://localhost:8080").await;
    blocked_ip_err.unwrap_err();

    let text = archive::fetch_url_as_text("https://google.com").await?;
    dbg!(&text);
    assert!(!text.is_empty());

    let text = archive::fetch_url_as_text("https://github.com/ArthurTent/ShaderAmp").await?;
    assert!(!text.is_empty());

    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "Test depends on an external resource and should only be run manually."]
async fn flaky_test_readability() -> anyhow::Result<()> {
    let url = "https://rafa.ee";
    let text = archive::fetch_url_as_text(url).await?;
    let readable = archive::make_readable(url.parse()?, &text)?;
    dbg!(readable);

    let url = "https://google.com";
    let text = archive::fetch_url_as_text(url).await?;
    let readable = archive::make_readable(url.parse()?, &text)?;
    dbg!(readable);

    let url = "https://github.com/ArthurTent/ShaderAmp";
    let text = archive::fetch_url_as_text(url).await?;
    let readable = archive::make_readable(url.parse()?, &text)?;
    dbg!(readable);

    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "Test depends on an external resource and should only be run manually."]
async fn flaky_test_archive_queue() -> anyhow::Result<()> {
    let app = TestApp::new().await;

    let user = app.create_test_user().await;
    let bookmark = app.create_bookmark(&user, "https://rafa.ee", "test").await;

    let mut tx = app.tx().await;

    let archive = db::archives::insert_pending(&mut tx, bookmark.id).await?;
    tx.commit().await?;

    let queue = archive::QueueHandle::new(app.pool.clone());

    let processed = queue.wait_until_archive_processed(archive.id);
    queue.archive_in_background(archive.id);
    assert!(processed.await);

    let mut tx = app.tx().await;
    let archive = db::archives::by_bookmark_id(&mut tx, archive.bookmark_id)
        .await?
        .unwrap();
    assert_eq!(archive.status, db::archives::Status::Success);

    Ok(())
}

#[test_log::test(tokio::test)]
#[ignore = "Test depends on an external resource and should only be run manually."]
async fn flaky_test_archive_queue_dangling_pending() -> anyhow::Result<()> {
    let app = TestApp::new().await;

    let user = app.create_test_user().await;
    let bookmark = app.create_bookmark(&user, "https://rafa.ee", "test").await;

    let mut tx = app.tx().await;
    let archive = db::archives::insert_pending(&mut tx, bookmark.id).await?;
    tx.commit().await?;

    let queue = archive::QueueHandle::new(app.pool.clone());

    let processed = queue.wait_until_archive_processed(archive.id).await;
    assert!(processed);

    let mut tx = app.tx().await;
    let archive = db::archives::by_bookmark_id(&mut tx, archive.bookmark_id)
        .await?
        .unwrap();
    assert_eq!(archive.status, db::archives::Status::Success);

    Ok(())
}
