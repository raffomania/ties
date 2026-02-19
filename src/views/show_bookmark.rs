use htmf::prelude_inline::*;

use crate::{
    db,
    views::{content, layout},
};

pub struct Data {
    pub layout: layout::Template,
    pub bookmark: db::Bookmark,
    pub archive: Option<db::Archive>,
    pub backlinks: Vec<db::List>,
}

pub fn view(
    Data {
        layout,
        bookmark,
        archive,
        backlinks,
    }: Data,
) -> Element {
    layout::layout(
        fragment([
            header(
                class("bg-neutral-900 px-4 pt-3 pb-4"),
                [
                    h1(class("text-2xl tracking-tight font-bold"), bookmark.title),
                    content::link_url(&bookmark.url),
                    backlink_section(&backlinks),
                    archive_status(archive.as_ref()),
                ],
            ),
            div(class("border-b border-black"), ()),
            div(class("border-b border-neutral-700"), ()),
            archive_contents(archive.as_ref()),
        ]),
        &layout,
    )
}

fn archive_status(archive: Option<&db::Archive>) -> Element {
    let Some(archive) = archive else {
        return p((), "Not archived yet");
    };

    p(
        (),
        format!("Archived on {}", content::format_date(archive.created_at)),
    )
}

fn archive_contents(archive: Option<&db::Archive>) -> Element {
    div(
        class("prose prose-invert px-4"),
        archive
            .and_then(|a| a.extracted_html.as_ref())
            .map_or(nothing(), unsafe_raw_html),
    )
}

fn backlink_section(backlinks: &[db::List]) -> Element {
    if backlinks.is_empty() {
        return nothing();
    }

    let link_elems = itertools::intersperse(
        backlinks.iter().map(|list| {
            fragment(a(
                [
                    href(format!("/lists/{}", list.id)),
                    class("hover:text-fuchsia-300"),
                ],
                &list.title,
            ))
        }),
        span((), " ∙ "),
    )
    .collect::<Vec<_>>();

    section(
        class("pb-4 mt-4"),
        [
            h2(
                class("font-bold mb-0.5 text-sm tracking-tight flex gap-1"),
                [
                    span((), "Backlinks"),
                    span(
                        [
                            title_attr("Backlinks are lists that point to this bookmark."),
                            class("text-neutral-400 hover:text-neutral-200 cursor-default text-sm"),
                        ],
                        "🛈",
                    ),
                ],
            ),
            p((), link_elems),
        ],
    )
}
