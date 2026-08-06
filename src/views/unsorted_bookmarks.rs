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
                header(class("px-4 pt-3 pb-4 dark:bg-neutral-900 bg-stone-100"))
                    .with([h1(class("text-xl font-bold"))
                        .with(format!("{} unsorted Bookmarks", data.bookmarks.len()))]),
            ])
            .with(layout::upper_border())
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
        "flex flex-wrap items-end justify-between gap-2 p-4 border-b last:border-b-0 \
         dark:border-neutral-700 border-neutral-300",
    ))
    .with([
        div(()).with([
            a([
                href(format!("/bookmarks/{bookmark_id}")),
                class(
                    "block overflow-hidden leading-8 dark:text-orange-100 text-orange-900 \
                     dark:hover:text-orange-300 hover:text-orange-700 shrink text-ellipsis \
                     whitespace-nowrap",
                ),
            ])
            .with(content::bookmark_title(&bookmark.title)),
            content::link_url(&bookmark.url),
        ]),
        div(class(
            "flex justify-end gap-2 grow dark:text-neutral-300 text-neutral-600",
        ))
        .with([a([
            href(format!("/links/create?dest_id={bookmark_id}")),
            class(
                "px-4 py-1 border rounded dark:border-neutral-700 border-neutral-300 \
                 dark:hover:bg-neutral-700 hover:bg-neutral-200",
            ),
        ])
        .with([
            text("Add to list"),
            a([
                attr("hx-delete", format!("/bookmarks/{bookmark_id}")),
                href(format!("/bookmarks/{bookmark_id}")),
                class(
                    "px-4 py-1 border rounded dark:border-neutral-700 border-neutral-300 \
                     dark:hover:bg-neutral-600 hover:bg-neutral-200",
                ),
            ])
            .with([text("Delete")]),
        ])]),
    ])
}
