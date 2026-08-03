use htmf::prelude::*;

use super::layout;
use crate::{form_errors::FormErrors, forms};

pub struct Data {
    pub layout: layout::Template,

    pub errors: FormErrors,
    pub input: forms::bookmarks::CreateBookmark,
}

pub fn view(
    Data {
        layout,
        errors,
        input: input_data,
    }: &Data,
) -> Element {
    layout::layout(
        fragment().with([
            layout::upper_border(),
            form([
                action("/bookmarks/create"),
                class("flex flex-col max-w-xl px-4 mt-4 md:mt-8 mx-auto mb-4 grow"),
                attr("hx-post", "/bookmarks/create"),
                attr("hx-push-url", "true"),
                attr("hx-select", "main"),
                attr("hx-target", "main"),
                id("create_bookmark"),
                method("POST"),
            ])
            .with([
                header(class("mt-3 mb-4"))
                    .with([h1(class("text-xl font-bold")).with("Add a bookmark")]),
                label(for_("url")).with("URL"),
                errors.view("url"),
                input([
                    value(&input_data.url),
                    class(
                        "rounded py-1.5 px-3 mt-2 bg-white border border-neutral-300 \
                         dark:bg-neutral-900 dark:border-neutral-700",
                    ),
                    name("url"),
                    placeholder("https://..."),
                    required(""),
                    type_("text"),
                ]),
                errors.view("root"),
                button([
                    class("bg-neutral-300 py-1.5 px-3 text-neutral-900 rounded mt-4 self-end"),
                    attr("hx-post", "/bookmarks/create"),
                    attr("hx-select-oob", "#nav"),
                    name("submitted"),
                    type_("submit"),
                    value("true"),
                ])
                .with("Add Bookmark"),
            ]),
        ]),
        layout,
    )
}
