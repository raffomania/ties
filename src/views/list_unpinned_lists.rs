use std::ops::Not;

use htmf::prelude_inline::*;

use crate::db;

pub struct Data {
    pub layout: super::layout::Template,
    pub lists: Vec<db::lists::UnpinnedList>,
}

pub fn view(Data { layout, lists }: Data) -> Element {
    super::layout::layout(
        [
            header(
                class("px-4 pt-3 pb-4 bg-neutral-900 border-b border-black"),
                [h1(class("text-xl font-bold"), "Unpinned Lists")],
            ),
            fragment(lists.into_iter().map(list_item).collect::<Vec<_>>()),
        ],
        &layout,
    )
}

fn list_item(list: db::lists::UnpinnedList) -> Element {
    section(
        class(
            "flex flex-wrap items-end justify-between gap-2 px-4 pt-4 pb-4 border-t \
             border-neutral-700",
        ),
        [div(
            class("overflow-hidden"),
            [
                a(
                    [
                        class(
                            "block overflow-hidden font-semibold leading-8 hover:text-fuchsia-300 \
                             text-ellipsis whitespace-nowrap",
                        ),
                        href(format!("/lists/{}", list.id)),
                    ],
                    list.title,
                ),
                list.content
                    .and_then(|content| {
                        content
                            .is_empty()
                            .not()
                            .then_some(p(class("mt-2"), content))
                    })
                    .unwrap_or(nothing()),
                div(
                    class("flex flex-wrap text-sm gap-x-2 text-neutral-400"),
                    [
                        p((), format!("{} bookmarks", list.bookmark_count)),
                        text("∙"),
                        p((), format!("{} lists", list.linked_list_count)),
                    ],
                ),
            ],
        )],
    )
}
