use htmf::prelude_inline::*;
use uuid::Uuid;

use crate::{form_errors::FormErrors, forms};

pub struct Data {
    pub layout: super::layout::Template,
    pub form_input: forms::lists::EditTitle,
    pub errors: FormErrors,
    pub list_id: Uuid,
}

pub fn view(
    Data {
        layout,
        form_input,
        errors,
        list_id,
    }: Data,
) -> Element {
    super::layout::layout(
        [
            div(class("border-t dark:border-black border-neutral-200"), ()),
            div(
                class("border-t dark:border-neutral-700 border-neutral-300"),
                (),
            ),
            form(
                [
                    action(format!("/lists/{list_id}/edit_title")),
                    class("flex flex-col max-w-xl mx-4 mb-4 grow"),
                    method("POST"),
                ],
                [
                    header(
                        class("mt-3 mb-4"),
                        [h1(class("text-xl font-bold"), "Rename list")],
                    ),
                    label(for_("title"), "New title"),
                    errors.view("title"),
                    input([
                        value(form_input.title),
                        class(
                            "rounded py-1.5 px-3 mt-2 bg-white border border-neutral-300 \
                             dark:bg-neutral-900 dark:border-neutral-700",
                        ),
                        name("title"),
                        required(""),
                        type_("text"),
                    ]),
                    errors.view("root"),
                    button(
                        [
                            class(
                                "bg-neutral-300 py-1.5 px-3 text-neutral-900 rounded mt-4 self-end",
                            ),
                            type_("submit"),
                        ],
                        "Save Changes",
                    ),
                ],
            ),
        ],
        &layout,
    )
}
