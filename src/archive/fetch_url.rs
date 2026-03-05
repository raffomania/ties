use core::f64;

use anyhow::Result;
use easy_cast::Conv;
use encoding_rs::{Encoding, UTF_8};
use http_body_util::BodyExt;
use mime_guess::Mime;
use url::Url;

use crate::archive::{self, safe_ips};

const MAX_RESPONSE_SIZE_BYTES: u64 = 5 * 1000 * 1000; // ~ 5 megabytes
const MAX_RESPONSE_SIZE_BYTES_USIZE: usize = 5 * 1000 * 1000; // ~ 5 megabytes

fn is_domain_url(url: &Url) -> bool {
    if let Some(host) = url.host() {
        match host {
            url::Host::Ipv4(_) | url::Host::Ipv6(_) => false,
            url::Host::Domain(_) => true,
        }
    } else {
        false
    }
}

pub async fn fetch_url_as_text(unvalidated_url: &str) -> Result<String, archive::Error> {
    let url = Url::parse(unvalidated_url)?;

    // Do not allow protocols other than http/s - this can pose a security risk, and
    // ties is meant to be used with http-based websites.
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(archive::Error::UnsupportedScheme {
                scheme: scheme.to_string(),
            });
        }
    }

    // Do not allow URLs pointing to IPs directly - this can pose a security risk,
    // and ties is meant to be used with domain-based websites.
    if !is_domain_url(&url) {
        return Err(archive::Error::IpUrl);
    }

    let redirect_policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() > 5 {
            attempt.error("Too many redirects")
        } else if !is_domain_url(attempt.url()) {
            attempt.error(archive::Error::IpUrl)
        } else {
            attempt.follow()
        }
    });

    // TODO include version in user agent, or use a different user agent that won't
    // get us blocked on so many sites
    let client = reqwest::Client::builder()
        .user_agent("ties")
        .dns_resolver(safe_ips::SafeDnsResolver)
        .redirect(redirect_policy)
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let response = client.get(url).send().await?.error_for_status()?;

    if let Some(length_according_to_header) = response.content_length()
        && length_according_to_header > MAX_RESPONSE_SIZE_BYTES
    {
        return Err(archive::Error::ResponseTooLarge {
            actual_size_mb: f64::try_conv(length_according_to_header).unwrap_or(f64::MAX)
                / 1_000_000.0,
        });
    }

    tracing::debug!(headers = ?response.headers());

    // Check that the content type is supported
    let content_type = response
        .headers()
        .get("content-type")
        .ok_or(archive::Error::UnexpectedInternal)?
        .to_str()
        .map_err(|_| archive::Error::UnexpectedInternal)?
        .to_string();
    if !content_type.starts_with("text/html") {
        return Err(archive::Error::UnsupportedContentType { content_type });
    }

    limited_body_to_text(response).await
}

async fn limited_body_to_text(response: reqwest::Response) -> Result<String, archive::Error> {
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<Mime>().ok());
    let limited_body =
        http_body_util::Limited::new(reqwest::Body::from(response), MAX_RESPONSE_SIZE_BYTES_USIZE);
    let encoding_name = content_type
        .as_ref()
        .and_then(|mime| mime.get_param("charset").map(|charset| charset.as_str()))
        .unwrap_or("utf-8");
    let encoding = Encoding::for_label(encoding_name.as_bytes()).unwrap_or(UTF_8);

    let full = BodyExt::collect(limited_body)
        .await
        .map(|buf| buf.to_bytes())
        .map_err(|_| archive::Error::UnexpectedInternal)?;

    let (text, _, _) = encoding.decode(&full);

    Ok(text.to_string())
}
