use htmf::{attr::Attrs, prelude_inline::*};
use url::Url;

use super::layout;
use crate::db::layout::AuthedInfo;

pub struct Data<'a> {
    pub layout: &'a layout::Template,
    pub base_url: &'a Url,
    pub authed_info: &'a AuthedInfo,
}

pub fn view(data: &Data) -> Element {
    super::layout::layout(
        [
            div(class("border-t border-black"), ()),
            div(class("border-t border-neutral-700"), ()),
            div(
                class("px-4 flex flex-col w-full items-center"),
                [
                    header(
                        class("m-8"),
                        [h1(
                            class("text-xl font-bold flex items-center gap-2"),
                            [
                                img([src("/assets/logo_icon_only.png"), class("inline h-8")]),
                                span(
                                    (),
                                    format!("Welcome to ties, {}!", data.authed_info.username),
                                ),
                            ],
                        )],
                    ),
                    // TODO add intro text: what can you do with ties? How to get started?  Where
                    // to get help?
                    div(
                        class(
                            "flex flex-wrap gap-x-2 gap-y-4 justify-center pb-4 text-center w-full",
                        ),
                        [
                            div(
                                class("flex flex-col gap-2 w-72"),
                                [
                                    dash_button(href("/bookmarks/create"), "Add a bookmark"),
                                    dash_button(href("/lists/create"), "Create a list"),
                                ],
                            ),
                            div(
                                class("flex flex-col gap-2"),
                                [
                                    dash_button(
                                        href(format!("/user/{}", data.authed_info.username)),
                                        "View my profile",
                                    ),
                                    form(
                                        [action("/logout"), method("post")],
                                        button(
                                            class(
                                                "w-full block p-4 border rounded \
                                                 border-neutral-700 hover:bg-neutral-700",
                                            ),
                                            "Logout",
                                        ),
                                    ),
                                ],
                            ),
                        ],
                    ),
                    bookmarklet_section(data),
                    // TODO add social links here
                ],
            ),
        ],
        data.layout,
    )
}

fn dash_button<C: Into<Element>>(attrs: Attrs, children: C) -> Element {
    a(
        [
            class("block px-8 py-4 w-full border rounded border-neutral-700 hover:bg-neutral-700"),
            attrs,
        ],
        children,
    )
}

pub fn bookmarklet_section(data: &Data) -> Element {
    fragment([
        header(
            class("pt-8"),
            [h2(class("font-bold"), "Install Bookmarklet")],
        ),
        section(
            class("pt-2 pb-4"),
            [bookmarklet_help(), bookmarklet(data.base_url)],
        ),
    ])
}

fn bookmarklet_help() -> Element {
    fragment([
        p(
            class("mb-2"),
            "Click the bookmarklet on any website to add it as a bookmark in
      ties!",
        ),
        p(
            [class("mb-4")],
            "To install, drag the following link to your bookmarks / favorites toolbar:",
        ),
    ])
}

fn bookmarklet(base_url: &Url) -> Element {
    // window.open(
    //   "{ base_url }bookmarks/create?url="
    //   +encodeURIComponent(window.location.href)
    //   +"&title="
    //   +encodeURIComponent(document.title)
    // )
    a(
        [
            class(
                "text-center block my-2 font-bold text-orange-200 border rounded py-2 px-16 \
                 cursor-grab",
            ),
            href(format!(
                "javascript:(function()%7Bwindow.open(%0A%20%20%22{base_url}bookmarks%2Fcreate%\
                 3Furl%3D%22%0A%20%20%2BencodeURIComponent(window.location.href)%0A%20%20%2B%22%\
                 26title%3D%22%0A%20%20%2BencodeURIComponent(document.title)%0A)%7D)()",
            )),
        ],
        "Add to ties",
    )
}
