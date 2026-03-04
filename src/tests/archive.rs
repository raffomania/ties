use crate::{archive, db, tests::util::test_app::TestApp};

#[test_log::test(tokio::test)]
#[ignore = "Test depends on an external resource and should only be run manually."]
async fn flaky_test_get_website() -> anyhow::Result<()> {
    let text = archive::fetch_url_as_text("https://rafa.ee").await?;
    assert!(!text.is_empty());

    let image_err = archive::fetch_url_as_text("https://www.rafa.ee/portrait.jpg").await;
    assert!(image_err.is_err());

    let blocked_ip_err = archive::fetch_url_as_text("https://localhost:8080").await;
    assert!(blocked_ip_err.is_err());

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
    let mut tx = app.tx().await;
    let bookmark = db::bookmarks::insert_local(
        &mut tx,
        user.ap_user_id,
        db::bookmarks::InsertBookmark {
            url: "https://rafa.ee".to_string(),
            title: "test".to_string(),
        },
        &app.base_url,
    )
    .await?;

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
    let mut tx = app.tx().await;
    let bookmark = db::bookmarks::insert_local(
        &mut tx,
        user.ap_user_id,
        db::bookmarks::InsertBookmark {
            url: "https://rafa.ee".to_string(),
            title: "test".to_string(),
        },
        &app.base_url,
    )
    .await?;

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
