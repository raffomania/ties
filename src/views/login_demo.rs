use htmf::prelude_inline::*;

use super::base_document::base_document;

pub fn view() -> Element {
    base_document(form(
        [
            action("/login_demo"),
            class("flex flex-col justify-center flex-1 max-w-md min-h-full px-4 mx-auto"),
            attr("hx-boost", "true"),
            attr("hx-disabled-elt", "button"),
            method("post"),
        ],
        [
            h1(
                class("text-2xl font-bold tracking-tight text-center"),
                "Welcome to the ties demo!",
            ),
            p(
                class("mt-10"),
                "Here, you can try ties with a temporary account. Every hour, All accounts on \
                 this server are permanently deleted.",
            ),
            button(
                [
                    class(
                        "leading-6 bg-neutral-300 mt-5 font-semibold rounded py-1.5 flex \
                         items-center justify-center disabled:bg-neutral-500 text-neutral-900",
                    ),
                    type_("submit"),
                ],
                [
                    span(
                        class("inline-block w-0 h-4"),
                        [span(
                            class(
                                "block w-4 h-4 -ml-6 border-2 rounded-full border-neutral-900 \
                                 animate-spin border-t-transparent htmx-indicator",
                            ),
                            (),
                        )],
                    ),
                    text("Try using a temporary account"),
                ],
            ),
        ],
    ))
}
