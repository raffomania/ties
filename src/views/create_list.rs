use htmf::{into_attrs::IntoAttrs, prelude::*};

use super::layout;
use crate::{form_errors::FormErrors, forms::lists::CreateList};

pub struct Data {
    pub layout: layout::Template,
    pub input: CreateList,
    pub errors: FormErrors,
}

pub fn view(
    Data {
        layout,
        input: input_data,
        errors,
    }: &Data,
) -> Element {
    layout::layout(
        div(class("border-t border-black")).with([
            div(class("border-t border-neutral-700")),
            form([
                action("/lists/create"),
                method("POST"),
                class("flex flex-col max-w-xl mx-4 mb-4 grow"),
            ])
            .with([
                header(class("mt-3 mb-4"))
                    .with([h1(class("text-xl font-bold")).with("Create a list")]),
                label(for_("title")).with("Title"),
                errors.view("title"),
                input([
                    required(""),
                    name("title"),
                    type_("text"),
                    value(&input_data.title),
                    class("rounded py-1.5 px-3 mt-2 bg-neutral-900"),
                ]),
                label(class("mt-4")).with([
                    text("Note"),
                    errors.view("content"),
                    textarea([
                        name("content"),
                        placeholder(""),
                        value(input_data.content.as_deref().unwrap_or("")),
                        class("rounded py-1.5 px-3 mt-2 bg-neutral-900 block w-full"),
                    ]),
                ]),
                div(class("mt-3 mb-5")).with([label(()).with([
                    input([
                        type_("checkbox"),
                        name("private"),
                        value("true"),
                        input_data.private.then(checked).into_attrs(),
                    ]),
                    text(" Private"),
                ])]),
                errors.view("root"),
                button([
                    type_("submit"),
                    class("bg-neutral-300 py-1.5 px-3 text-neutral-900 rounded mt-4 self-end"),
                ])
                .with("Add List"),
            ]),
        ]),
        layout,
    )
}
