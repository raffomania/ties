use anyhow::{Context, Result, anyhow};
use url::Url;

use crate::archive::safe_dns_resolver;

const MAX_RESPONSE_SIZE_BYTES: u64 = 5 * 1000 * 1000; // ~ 5 megabytes

pub async fn fetch_url_as_text(unvalidated_url: &str) -> Result<String> {
    let url = Url::parse(unvalidated_url)?;

    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(anyhow!(
                "URL scheme '{scheme}' not supported (only http or https are supported)"
            ));
        }
    }

    // TODO include version in user agent, or use a different user agent that won't
    // get us blocked on so many sites
    let client = reqwest::Client::builder()
        .user_agent("ties")
        .dns_resolver(safe_dns_resolver::SafeDnsResolver)
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let response = client.get(url).send().await?.error_for_status()?;

    if let Some(length_according_to_header) = response.content_length()
        && length_according_to_header > MAX_RESPONSE_SIZE_BYTES
    {
        return Err(anyhow!(
            "Response was larger than the supported maximum of {MAX_RESPONSE_SIZE_BYTES} bytes."
        ));
    }

    tracing::debug!(headers = ?response.headers());

    // Check that the content type is supported
    let content_type = response
        .headers()
        .get("content-type")
        .context("Missing content-type header")?
        .to_str()?;
    if !content_type.starts_with("text/html") {
        return Err(anyhow!("Content type '{content_type}' not supported"));
    }

    let text = response.text().await?;

    if text.len().try_into().unwrap_or(u64::MAX) > MAX_RESPONSE_SIZE_BYTES {
        return Err(anyhow!(
            "Response was larger than the supported maximum of {MAX_RESPONSE_SIZE_BYTES} bytes."
        ));
    }

    Ok(text)
}
