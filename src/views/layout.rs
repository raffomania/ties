use htmf::{into_elements::IntoElements, prelude::*};

use super::base_document::base_document;
use crate::{
    authentication::AuthUser,
    db::{self, AppTx, List, layout::AuthedInfo},
    response_error::ResponseResult,
};

pub struct Template {
    pub authed_info: Option<AuthedInfo>,
}

impl Template {
    pub async fn from_db(tx: &mut AppTx, auth_user: Option<&AuthUser>) -> ResponseResult<Self> {
        let auth_info = if let Some(auth_user) = auth_user {
            Some(db::layout::by_ap_user_id(tx, auth_user.ap_user_id).await?)
        } else {
            None
        };
        Ok(Template {
            authed_info: auth_info,
        })
    }
}

pub fn layout<Children: IntoElements>(children: Children, layout: &Template) -> Element {
    base_document(div(class("flex-row-reverse h-full sm:flex")).with([
        main_(class("sm:overflow-y-auto sm:grow")).with(children),
        match &layout.authed_info {
            Some(info) => sidebar(info),
            None => fragment(),
        },
    ]))
}

fn sidebar(authed_info: &AuthedInfo) -> Element {
    aside([
        id("nav"),
        class(
            "bg-neutral-900 sm:max-w-[18rem] sm:w-1/3 sm:max-h-full flex flex-col \
             sm:flex-col-reverse sm:border-r border-neutral-700 border-t sm:border-t-0",
        ),
    ])
    .with([
        div(class("sm:overflow-y-auto sm:flex-1")).with([lists_header(), lists(authed_info)]),
        header(class(
            "sticky bottom-0 flex justify-between items-center p-2 leading-8 bg-neutral-900",
        ))
        .with([a([
            href(format!("/user/{}", authed_info.username)),
            class("px-2 font-bold rounded hover:bg-neutral-800"),
        ])
        .with([span([]).with(&authed_info.username)])]),
    ])
}

fn lists_header() -> Element {
    div(class(
        "sticky top-0 flex items-center justify-between px-2 pt-2 sm:top-0 bg-neutral-900",
    ))
    .with([
        h3(class(
            "px-2 py-1 text-sm font-bold tracking-tight text-neutral-400",
        ))
        .with("Lists"),
        a([
            href("/lists/create"),
            class("block px-3 text-xl rounded hover:bg-neutral-800 text-neutral-400"),
        ])
        .with("+"),
    ])
}

fn lists(authed_info: &AuthedInfo) -> Element {
    let lists = authed_info.lists.iter();
    ul(class("pb-2")).with([
        li([]).with(
            a([
                class(
                    "block px-4 py-1 overflow-hidden text-ellipsis whitespace-nowrap \
                     hover:bg-neutral-800",
                ),
                href("/bookmarks/unsorted"),
            ])
            .with("Unsorted bookmarks"),
        ),
        fragment().with(lists.map(list_item).collect::<Vec<_>>()),
        li([]).with(
            a([
                class(
                    "block px-4 py-1 overflow-hidden text-ellipsis whitespace-nowrap \
                     hover:bg-neutral-800 text-neutral-400",
                ),
                href("/lists/unpinned"),
            ])
            .with("Unpinned lists"),
        ),
    ])
}

fn list_item(list: &List) -> Element {
    li([]).with(
        a([
            class(
                "block px-4 py-1 overflow-hidden text-ellipsis whitespace-nowrap \
                 hover:bg-neutral-800",
            ),
            href(format!("/lists/{}", list.id)),
        ])
        .with(&list.title),
    )
}
