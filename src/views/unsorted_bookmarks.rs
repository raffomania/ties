use htmf::prelude::*;

use super::{content, layout};
use crate::db::{self, Bookmark};

pub struct Data {
    pub layout: layout::Template,
    pub bookmarks: Vec<db::Bookmark>,
}

pub fn view(data: &Data) -> Element {
    layout::layout(
        fragment()
            .with([
                header(class("px-4 pt-3 pb-4 bg-neutral-900 border-b border-black"))
                    .with([h1(class("text-xl font-bold")).with("Unsorted Bookmarks")]),
            ])
            .with(
                data.bookmarks
                    .iter()
                    .map(bookmark_entry)
                    .collect::<Vec<Element>>(),
            ),
        &data.layout,
    )
}

fn bookmark_entry(bookmark: &Bookmark) -> Element {
    let bookmark_id = bookmark.id;

    section(class(
        "flex flex-wrap items-end justify-between gap-2 py-4 border-t border-neutral-700",
    ))
    .with([
        a([
            href(&bookmark.url),
            class(
                "block px-4 overflow-hidden leading-8 text-orange-100 hover:text-orange-300 \
                 shrink text-ellipsis whitespace-nowrap",
            ),
        ])
        .with(&bookmark.title)
        .with(content::link_url(&bookmark.url)),
        div(class("flex justify-end gap-2 mx-4 grow text-neutral-300")).with([a([
            href(format!("/links/create?dest_id={bookmark_id}")),
            class("px-4 py-1 border rounded border-neutral-700 hover:bg-neutral-700"),
        ])
        .with([
            text("Add to list"),
            a([
                attr("hx-delete", format!("/bookmarks/{bookmark_id}")),
                href(format!("/bookmarks/{bookmark_id}")),
                class("px-4 py-1 border rounded border-neutral-700 hover:bg-neutral-600"),
            ])
            .with([text("Delete")]),
        ])]),
    ])
}
