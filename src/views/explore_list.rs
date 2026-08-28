use crate::{
    db,
    views::{
        content::{self, pluralize},
        layout,
    },
};
use htmf::prelude::*;

pub struct Data {
    pub layout: layout::Template,
    pub list: db::List,
    pub metadata: db::lists::Metadata,
    /// Sorted.
    pub items: Vec<db::Activity>,
}

pub fn view(
    Data {
        layout,
        list,
        metadata,
        items,
    }: &Data,
) -> Element {
    layout::layout(
        fragment()
            .with(
                div(class("dark:bg-neutral-900 bg-stone-100 px-4 pb-4"))
                    .with(title_and_description(list, metadata)),
            )
            .with(layout::upper_border())
            .with(
                items
                    .iter()
                    .map(|item| list_item_bookmark(&layout, &item))
                    .collect::<Vec<_>>(),
            )
            .with(
                items.is_empty().then_some(
                    p(class("dark:text-neutral-400 text-neutral-500 italic p-4"))
                        .with("No bookmarks here yet."),
                ),
            ),
        layout,
    )
}

fn title_and_description(list: &db::List, metadata: &db::lists::Metadata) -> Element {
    header(class("pt-3 mb-4")).with([
        h1(class("text-2xl font-bold tracking-tight"))
            .with(
                a([
                    href(list.path()),
                    class("hover:text-neutral-900 text-neutral-600"),
                ])
                .with(&list.title),
            )
            .with(span(class("text-neutral-600")).with(" » "))
            .with("Activity"),
        div(class(
            "flex flex-wrap text-sm gap-x-1 dark:text-neutral-400 text-neutral-500",
        ))
        .with([
            a([
                href(format!("/user/{}", metadata.username)),
                class("dark:hover:text-neutral-200 hover:text-neutral-600"),
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
}

fn list_item_bookmark(layout: &layout::Template, activity: &db::Activity) -> Element {
    section(class(
        "flex flex-wrap items-end gap-2 px-4 pt-3 pb-4 border-b last:border-b-0 \
         dark:border-neutral-700 border-neutral-300",
    ))
    .with([
        div(class("overflow-hidden")).with([
            a([
                class(
                    "block overflow-hidden leading-8 dark:text-orange-100 text-orange-900 \
                 dark:hover:text-orange-300 hover:text-orange-700 text-ellipsis whitespace-nowrap",
                ),
                href(format!("/bookmarks/{}", activity.bookmark_id)),
            ])
            .with(content::bookmark_title(&activity.title)),
            content::link_url(&activity.url),
        ]),
        div(class("flex flex-wrap flex-1 items-end flex-col")).with([
            p(class("py-1 text-neutral-500 dark:text-neutral-400 text-sm")).with([
                a([
                    class("text-neutral-600 dark:text-neutral-300 hover:text-neutral-700 dark:hover:text-neutral-200"),
                    href(format!("/user/{}", activity.username)),
                ])
                .with(&activity.username),
                span(class("italic")).with(" on "),
                span(class("italic")).with(content::format_date(activity.created_at)),
            ]),
            if layout.authed_info.is_some() {
                div(class(
                    "text-sm gap-x-2 dark:text-neutral-400 text-neutral-500",
                ))
                .with([a([
                    class("dark:hover:text-neutral-100 hover:text-neutral-800"),
                    href(format!("/links/create?dest_id={}", activity.bookmark_id)),
                ])
                .with("Connect bookmark")])
            } else {
                nothing()
            },
        ]),
    ])
}
