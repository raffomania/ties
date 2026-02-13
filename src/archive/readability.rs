use anyhow::Result;

pub fn make_readable(html: &str) -> Result<legible::Article> {
    let article = legible::parse(html, None, None)?;
    Ok(article)
}
