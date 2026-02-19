use htmf::prelude_inline::*;
use time::{OffsetDateTime, format_description};

pub fn link_url(url: &str) -> Element {
    p(
        class(
            "w-full overflow-hidden text-sm text-neutral-400 hover:text-neutral-300 whitespace-nowrap text-ellipsis",
        ),
        a(href(url), url),
    )
}

pub fn format_date(date: OffsetDateTime) -> String {
    date.format(&format_description::parse("[year]-[month]-[day]").unwrap())
        .unwrap()
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
