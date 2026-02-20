use htmf::prelude::*;

use super::{content, layout};
use crate::{
    db::{self, LinkWithContent},
    views::content::pluralize,
};

pub struct Data {
    pub layout: layout::Template,
    pub links: Vec<db::LinkWithContent>,
    pub list: db::List,
    pub metadata: db::lists::Metadata,
    pub backlinks: Vec<db::List>,
}

pub fn view(
    data @ Data {
        layout,
        links,
        list,
        metadata,
        backlinks,
    }: &Data,
) -> Element {
    layout::layout(
        fragment()
            .with(
                div(class("bg-neutral-900 border-b border-black px-4"))
                    .with(title_and_description(list, metadata))
                    .with(layout.authed_info.as_ref().and_then(|authed_info| {
                        (authed_info.ap_user_id == list.ap_user_id).then(|| edit_buttons(data))
                    }))
                    .with(backlink_section(backlinks)),
            )
            .with(
                links
                    .iter()
                    .map(|link| list_item(link, data))
                    .collect::<Vec<_>>(),
            )
            .with(
                links.is_empty().then_some(
                    p(class(
                        "border-t border-neutral-700 text-neutral-400 italic p-4",
                    ))
                    .with("No bookmarks here yet."),
                ),
            ),
        layout,
    )
}

fn title_and_description(list: &db::List, metadata: &db::lists::Metadata) -> Element {
    header(class("pt-3 mb-4"))
        .with([
            h1(class("text-xl font-bold tracking-tight")).with(&list.title),
            div(class("flex flex-wrap text-sm gap-x-1 text-neutral-400")).with([
                a([
                    href(format!("/user/{}", metadata.username)),
                    class("hover:text-neutral-200"),
                ])
                .with(format!("by {}", metadata.username)),
                text("∙"),
                p([]).with(format!("{} bookmarks", metadata.linked_bookmark_count)),
                text("∙"),
                p([]).with(pluralize(metadata.linked_list_count, "list", "lists")),
                text("∙"),
                p(id("private_indicator")).with(if list.private { "private" } else { "public" }),
            ]),
        ])
        .with(list.content.as_ref().and_then(|content| {
            (!content.is_empty()).then_some(p(class("max-w-2xl mt-2")).with(content))
        }))
}

fn backlink_section(backlinks: &[db::List]) -> Element {
    use htmf::prelude_inline::*;

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
                            title_attr(
                                "These are lists that point to the list you are currently viewing.",
                            ),
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

fn edit_buttons(Data { list, .. }: &Data) -> Element {
    section(class("flex flex-wrap my-4 mx-1 gap-x-4 gap-y-2")).with([
        a([
            class("block px-4 py-1 border rounded hover:bg-neutral-800 border-neutral-700 w-max"),
            href(format!("/links/create?dest_id={}", list.id)),
        ])
        .with("Add to other list"),
        form([
            action(format!("/lists/{}/edit_private", list.id)),
            attr("hx-post", format!("/lists/{}/edit_private", list.id)),
            attr("hx-select", "#edit_private"),
            attr("hx-select-oob", "#private_indicator"),
            attr("hx-target", "this"),
            id("edit_private"),
            method("post"),
        ])
        .with([button([
            class("block px-4 py-1 border rounded hover:bg-neutral-800 border-neutral-700 w-max"),
            name("private"),
            type_("submit"),
            value(if list.private { "false" } else { "true" }),
        ])
        .with(if list.private {
            "Make public"
        } else {
            "Make private"
        })]),
        form([
            action(format!("/lists/{}/edit_pinned", list.id)),
            id("edit_pinned"),
            method("post"),
        ])
        .with([button([
            class("block px-4 py-1 border rounded hover:bg-neutral-800 border-neutral-700 w-max"),
            name("pinned"),
            type_("submit"),
            value(if list.pinned { "false" } else { "true" }),
        ])
        .with(if list.pinned {
            "Unpin from sidebar"
        } else {
            "Pin to sidebar"
        })]),
        a([
            class("block px-4 py-1 border rounded hover:bg-neutral-800 border-neutral-700 w-max"),
            href(format!("/lists/{}/edit_title", list.id)),
        ])
        .with([
            text("Rename"),
            a([
                class(
                    "block px-4 py-1 border rounded hover:bg-neutral-800 border-neutral-700 w-max",
                ),
                href(format!("/bookmarks/create?parent_id={}", list.id)),
                attr("hx-get", format!("/bookmarks/create?parent_id={}", list.id)),
                attr("hx-select", "#create_bookmark"),
                attr("hx-target", "closest section"),
            ])
            .with("Add new bookmark"),
        ]),
    ])
}

fn list_item(link: &LinkWithContent, Data { layout, list, .. }: &Data) -> Element {
    section(class(
        "flex flex-wrap items-end gap-2 px-4 pt-4 pb-4 border-t border-neutral-700",
    ))
    .with([
        div(class("overflow-hidden")).with(match &link.dest {
            db::LinkDestinationWithChildren::List(inner_list) => list_item_list(inner_list),
            db::LinkDestinationWithChildren::Bookmark(bookmark) => list_item_bookmark(bookmark),
        }),
        if let Some(authed_info) = &layout.authed_info {
            div(class(
                "flex flex-wrap justify-end flex-1 pt-2 text-sm basis-32 gap-x-2 text-neutral-400",
            ))
            .with([
                a([
                    class("hover:text-neutral-100"),
                    href(format!("/links/create?dest_id={}", link.dest.id())),
                ])
                .with("Connect"),
                if authed_info.ap_user_id == list.ap_user_id {
                    fragment().with([
                        span(()).with("∙"),
                        button([
                            class("hover:text-neutral-100"),
                            attr("hx-delete", format!("/links/{}", link.id)),
                            attr("title", "Remove from list"),
                        ])
                        .with("Remove from list"),
                    ])
                } else {
                    nothing()
                },
            ])
        } else {
            nothing()
        },
    ])
}

fn list_item_bookmark(bookmark: &db::Bookmark) -> Element {
    fragment().with([
        a([
            class(
                "block overflow-hidden leading-8 text-orange-100 hover:text-orange-300 \
                 text-ellipsis whitespace-nowrap",
            ),
            href(&bookmark.url),
        ])
        .with(&bookmark.title),
        content::link_url(&bookmark.url),
    ])
}

fn list_item_list(inner_list: &db::ListWithLinks) -> Element {
    // TODO show owning user if it's different than this list's owner
    // https://github.com/raffomania/ties/issues/152
    fragment().with([
        a([
            class(
                "block overflow-hidden font-semibold leading-8 hover:text-fuchsia-300 \
                 text-ellipsis whitespace-nowrap",
            ),
            href(format!("/lists/{}", inner_list.list.id)),
        ])
        .with(&inner_list.list.title),
        fragment().with(inner_list.list.content.as_ref().and_then(|content| {
            (!content.is_empty()).then_some(p(class("max-w-2xl mb-2")).with(content))
        })),
        {
            let bookmark_count = inner_list
                .links
                .iter()
                .filter(|l| matches!(l, db::LinkDestination::Bookmark(_)))
                .count();
            let list_count = inner_list.links.len() - bookmark_count;
            div(class("text-sm text-neutral-400 flex flex-wrap gap-x-1")).with([
                p([]).with(pluralize(
                    bookmark_count.try_into().unwrap_or(-1),
                    "bookmark",
                    "bookmarks",
                )),
                text("∙"),
                p([]).with(pluralize(
                    list_count.try_into().unwrap_or(-1),
                    "list",
                    "lists",
                )),
            ])
        },
    ])
}
