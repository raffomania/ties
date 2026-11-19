use std::fmt::Write;

use anyhow::Context;
use url::Url;

#[derive(Debug)]
pub struct Resource {
    pub name: String,
    pub domain: String,
}

impl Resource {
    pub fn parse_handle(handle: &str, local_domain: &Url) -> anyhow::Result<Self> {
        match handle.split_once('@') {
            Some((name, domain)) => Ok(Resource {
                name: name.to_string(),
                domain: domain.to_string(),
            }),
            // Assume the handle is only a username, and assume it's located on
            // the local domain
            None => Self::from_name_and_url(handle.to_string(), local_domain),
        }
    }

    pub fn from_name_and_url(name: String, url: &Url) -> anyhow::Result<Self> {
        let mut domain = url
            .domain()
            .context("Missing domain for webfinger resource URL")?
            .to_string();
        if let Some(port) = url.port() {
            write!(domain, ":{port}")?;
        }
        Ok(Resource { name, domain })
    }
}
