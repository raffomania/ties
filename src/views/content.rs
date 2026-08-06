use anyhow::Context;
use htmf::prelude_inline::*;
use time::{OffsetDateTime, format_description};

pub static BULLET: &str = "∙";

pub fn link_url(url: &str) -> Element {
    p(
        class(
            "w-full overflow-hidden text-sm dark:text-neutral-400 text-neutral-500 \
             dark:hover:text-neutral-300 hover:text-neutral-800 whitespace-nowrap text-ellipsis",
        ),
        a(href(url), url),
    )
}

pub fn help_icon() -> Element {
    span(
        [class(
            "dark:text-neutral-400 text-neutral-500 dark:hover:text-neutral-200 \
             hover:text-neutral-600 cursor-default text-sm",
        )],
        "🛈",
    )
}

/// Return the title, or if it's None, a generic "untitled" title description.
#[expect(clippy::ref_option, reason = "It's easier to call this way.")]
pub fn bookmark_title(title: &Option<String>) -> &str {
    title.as_deref().unwrap_or("Untitled Bookmark")
}

pub fn format_date(date: OffsetDateTime) -> String {
    let maybe_formatted = format_description::parse("[year]-[month]-[day]")
        .context("Invalid date format description")
        .and_then(|fmt| date.format(&fmt).context("Failed to format date"));

    if let Err(e) = &maybe_formatted {
        tracing::error!(?e, "Failed to format date");
    }

    maybe_formatted.unwrap_or("failed to format date".to_string())
}

pub fn pluralize<'a>(
    count: i64,
    singular_description: &'a str,
    plural_description: &'a str,
) -> String {
    match count {
        1 => format!("{count} {singular_description}"),
        _ => format!("{count} {plural_description}"),
    }
}
