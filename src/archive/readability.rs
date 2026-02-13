use anyhow::Result;
use url::Url;

pub fn make_readable(base_url: Url, html: &str) -> Result<legible::Article> {
    let mut article = legible::parse(html, None, None)?;

    let mut ammonia_builder = ammonia::Builder::default();
    ammonia_builder.url_relative(ammonia::UrlRelative::RewriteWithBase(base_url));
    article.content = ammonia_builder.clean(&article.content).to_string();

    Ok(article)
}
