use htmf::{element::Element, prelude_inline::*};

use crate::{
    db,
    views::{content, layout},
};

pub struct Data {
    pub layout: layout::Template,
    pub results: db::search::Results,
}

pub fn view(data: &Data) -> Element {
    layout::layout(results(data), &data.layout)
}

// TODO use percent encoding for previous search input in urls

fn results(data: &Data) -> Element {
    fragment([
        p(
            class("bg-neutral-900 px-4 pt-3 pb-3 font-bold tracking-tight border-b border-black"),
            format!("{} bookmarks found", data.results.total_count),
        ),
        fragment(
            data.results
                .bookmarks
                .iter()
                .map(|r| list_item(r, data))
                .collect::<Vec<_>>(),
        ),
        pagination(data),
    ])
}

fn pagination(data: &Data) -> Element {
    section(
        class("flex flex-row gap-4 justify-center w-full p-4 border-t border-neutral-700"),
        [
            match data.results.previous_page {
                db::search::PreviousPage::AfterBookmarkId(id) => {
                    let url = format!(
                        "/search?q={}&after_bookmark_id={}",
                        data.layout.previous_search_input.as_deref().unwrap_or(""),
                        id
                    );
                    a([href(url)], "Previous page")
                }
                db::search::PreviousPage::IsFirstPage => {
                    let url = format!(
                        "/search?q={}",
                        data.layout.previous_search_input.as_deref().unwrap_or(""),
                    );
                    a([href(url)], "Previous page")
                }
                db::search::PreviousPage::DoesNotExist => nothing(),
            },
            match data.results.next_page_after_bookmark_id {
                Some(next_page_after_bookmark_id) => {
                    let url = format!(
                        "/search?q={}&after_bookmark_id={}",
                        data.layout.previous_search_input.as_deref().unwrap_or(""),
                        next_page_after_bookmark_id
                    );
                    a([href(url)], "Next page")
                }
                None => nothing(),
            },
        ],
    )
}

fn list_item(result: &db::search::Result, Data { layout, .. }: &Data) -> Element {
    section(
        class("flex flex-wrap items-end gap-2 px-4 pt-4 pb-4 border-t border-neutral-700"),
        [
            div(class("overflow-hidden"), list_item_bookmark(result)),
            if let Some(_authed_info) = &layout.authed_info {
                div(
                    class(
                        "flex flex-wrap justify-end flex-1 pt-2 text-sm basis-32 gap-x-2 \
                         text-neutral-400",
                    ),
                    [a(
                        [
                            class("hover:text-neutral-100"),
                            href(format!("/links/create?dest_id={}", result.bookmark_id)),
                        ],
                        "Connect",
                    )],
                )
            } else {
                nothing()
            },
        ],
    )
}

fn list_item_bookmark(result: &db::search::Result) -> Element {
    fragment([
        a(
            [
                class(
                    "block overflow-hidden leading-8 text-orange-100 hover:text-orange-300 \
                     text-ellipsis whitespace-nowrap",
                ),
                href(&result.bookmark_url),
            ],
            &result.title,
        ),
        content::link_url(&result.bookmark_url),
    ])
}
