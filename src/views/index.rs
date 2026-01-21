use htmf::prelude_inline::*;

use super::layout;

pub fn view(layout: &layout::Template) -> Element {
    super::layout::layout(
        fragment([
            header(
                class("mx-4 mt-3 mb-4"),
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
                                "block p-4 border rounded border-neutral-700 hover:bg-neutral-700",
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
                                "block p-4 border rounded border-neutral-700 hover:bg-neutral-700",
                            ),
                            href("/lists/create"),
                        ],
                        "Create a list",
                    ),
                )
                .with([li(
                    (),
                    a(
                        [
                            class(
                                "block px-4 py-2 border rounded border-neutral-700 \
                                 hover:bg-neutral-700",
                            ),
                            href("/profile"),
                        ],
                        "Install the bookmarklet",
                    ),
                )])])],
            ),
            // TODO add social links here
        ]),
        layout,
    )
}
