use htmf::prelude_inline::*;

use super::base_document::base_document;
use crate::{form_errors::FormErrors, forms::users::OidcSelectUsername};

#[derive(Default)]
pub struct Data {
    pub errors: FormErrors,
    pub form_input: OidcSelectUsername,
}

pub fn view(Data { errors, form_input }: Data) -> Element {
    base_document([div(
        class("flex flex-col justify-center max-w-md min-h-full px-4 mx-auto"),
        [form(
            [
                class("flex flex-col w-full"),
                attr("hx-boost", "true"),
                attr("hx-disabled-elt", "button"),
                method("post"),
            ],
            [
                h1(
                    class("text-2xl font-bold tracking-tight text-center"),
                    "Welcome to ties! Please select a username.",
                ),
                p(
                    (),
                    "It should consist of letters and numbers, and it can be 3 to 50 characters \
                     long. It will be your handle on the fediverse.",
                ),
                label(
                    [class("mt-10 text-neutral-400"), name("username")],
                    "Username",
                ),
                errors.view("username"),
                input([
                    class("rounded py-1.5 px-3 mt-2 bg-neutral-900"),
                    name("username"),
                    required(""),
                    type_("text"),
                    value(form_input.username),
                ]),
                errors.view("root"),
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
                            span(
                                class(
                                    "block w-4 h-4 -ml-6 border-2 rounded-full border-neutral-900 \
                                     animate-spin border-t-transparent htmx-indicator",
                                ),
                                (),
                            ),
                        ),
                        text("Sign in"),
                    ],
                ),
            ],
        )],
    )])
}
