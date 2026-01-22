use htmf::prelude_inline::*;
use url::Url;

use crate::{authentication::AuthUser, db::layout::AuthedInfo};

use super::layout;

pub struct Data<'a> {
    pub layout: &'a layout::Template,
    pub base_url: &'a Url,
    pub authed_info: &'a AuthedInfo,
}

pub fn view(data: &Data) -> Element {
    super::layout::layout(
        div(
            class("mx-4"),
            [
                header(
                    class("mt-3 mb-4 flex justify-between flex-wrap"),
                    [
                        h1(
                            class("text-xl font-bold flex items-center gap-2"),
                            [
                                img([src("/assets/logo_icon_only.png"), class("inline h-8")]),
                                span((), "Welcome to ties!"),
                            ],
                        ),
                        form(
                            [action("/logout"), method("post")],
                            button(
                                class("rounded px-3 py-1 text-neutral-400 hover:bg-neutral-700"),
                                "Log out",
                            ),
                        ),
                    ],
                ),
                // TODO add intro text: what can you do with ties? How to get started?  Where to
                // get help?
                ul(
                    class("flex flex-col max-w-sm gap-2 pb-4"),
                    [
                        li(
                            (),
                            a(
                                [
                                    class(
                                        "block p-4 border rounded border-neutral-700 hover:bg-neutral-700",
                                    ),
                                    href("/bookmarks/create"),
                                ],
                                "Add a bookmark",
                            ),
                        ),
                        li(
                            [],
                            a(
                                [
                                    class(
                                        "block p-4 border rounded border-neutral-700 hover:bg-neutral-700",
                                    ),
                                    href("/lists/create"),
                                ],
                                "Create a list",
                            ),
                        ),
                        li(
                            (),
                            a(
                                [
                                    class(
                                        "block p-4 border rounded border-neutral-700 hover:bg-neutral-700",
                                    ),
                                    href(format!("/user/{}", data.authed_info.username)),
                                ],
                                "View my profile",
                            ),
                        ),
                    ],
                ),
                // TODO add social links here
                bookmarklet_section(data),
            ],
        ),
        data.layout,
    )
}

pub fn bookmarklet_section(data: &Data) -> Element {
    fragment([
        header(
            class("pt-8"),
            [h2(class("font-bold"), "Install Bookmarklet")],
        ),
        section(
            class("py-2"),
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
    //   "{ base_url }/bookmarks/create?url="
    //   +encodeURIComponent(window.location.href)
    //   +"&title="
    //   +encodeURIComponent(document.title)
    // )
    a(
        [
            class(
                "text-center my-2 font-bold text-orange-200 border rounded py-2 px-16 cursor-grab",
            ),
            href(format!(
                "javascript:(function()%7Bwindow.open(%0A%20%20%22{base_url}%2Fbookmarks%2Fcreate%\
             3Furl%3D%22%0A%20%20%2BencodeURIComponent(window.location.href)%0A%20%20%2B%22%\
             26title%3D%22%0A%20%20%2BencodeURIComponent(document.title)%0A)%7D)()",
            )),
        ],
        "Add to ties",
    )
}
