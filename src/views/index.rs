use htmf::prelude_inline::*;
use url::Url;

use super::layout;

pub struct Data<'a> {
    pub layout: &'a layout::Template,
    pub base_url: &'a Url,
}

pub fn view(data: &Data) -> Element {
    super::layout::layout(
        div(
            class("border-t border-black"),
            [
                div(class("border-t border-neutral-700"), ()),
                header(
                    class("m-4"),
                    [h1(class("text-xl font-bold"), "Welcome to ties!")],
                ),
                // TODO add intro text: what can you do with ties? How to get started?  Where to
                // get help?
                ul(
                    class("flex flex-col max-w-sm gap-2 px-4 pb-4"),
                    [li(
                        (),
                        a(
                            [
                                class(
                                    "block p-4 border rounded border-neutral-700 \
                                     hover:bg-neutral-700",
                                ),
                                href("/bookmarks/create"),
                            ],
                            "Add a bookmark",
                        ),
                    )
                    .with([li(
                        [],
                        a(
                            [
                                class(
                                    "block p-4 border rounded border-neutral-700 \
                                     hover:bg-neutral-700",
                                ),
                                href("/lists/create"),
                            ],
                            "Create a list",
                        ),
                    )])
                    .with(bookmarklet_section(data))],
                ),
                // TODO add social links here
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
    //   "{ base_url }bookmarks/create?url="
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
                "javascript:(function()%7Bwindow.open(%0A%20%20%22{base_url}bookmarks%2Fcreate%\
                 3Furl%3D%22%0A%20%20%2BencodeURIComponent(window.location.href)%0A%20%20%2B%22%\
                 26title%3D%22%0A%20%20%2BencodeURIComponent(document.title)%0A)%7D)()",
            )),
        ],
        "Add to ties",
    )
}
