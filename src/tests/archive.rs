use crate::archive;

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
