use htmf::prelude::*;

use super::layout;
use crate::{db, form_errors::FormErrors, forms};

pub struct Data {
    pub layout: layout::Template,

    pub errors: FormErrors,
    pub input: forms::bookmarks::CreateBookmark,
    pub selected_parents: Vec<db::List>,
    pub search_results: Vec<db::List>,
}

pub fn view(
    data @ Data {
        layout,
        errors,
        input: input_data,
        selected_parents,
        ..
    }: &Data,
) -> Element {
    layout::layout(
        div(class("border-t border-black")).with([
            div(class("border-t border-neutral-700")),
            form([
                action("/bookmarks/create"),
                class("flex flex-col max-w-xl px-4 pb-4 rounded grow bg-neutral-800"),
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
                    class("rounded py-1.5 px-3 mt-2 bg-neutral-900"),
                    name("url"),
                    placeholder("https://..."),
                    required(""),
                    type_("text"),
                ]),
                label([class("mt-4"), for_("title")]).with("Title"),
                errors.view("title"),
                input([
                    value(&input_data.title),
                    class("rounded py-1.5 px-3 mt-2 bg-neutral-900"),
                    name("title"),
                    required(""),
                    type_("text"),
                ]),
                label([class("mt-4"), for_("list_search_term")]).with("Add to Lists"),
                div(id("selected_lists")).with([
                    errors.view("parents"),
                    fragment().with(
                        selected_parents
                            .iter()
                            .map(|parent| {
                                label(class("block leading-8 text-fuchsia-100")).with([
                                    span(class("text-fuchsia-100"))
                                        .with(format!("🧵 {}", parent.title)),
                                    input([name("parents[]"), type_("hidden"), value(parent.id)]),
                                ])
                            })
                            .collect::<Vec<_>>(),
                    ),
                    errors.view("create_parents"),
                    fragment().with(
                        input_data
                            .create_parents
                            .iter()
                            .map(|parent_name| {
                                label(class("block leading-8")).with([
                                    text("New public list "),
                                    span(class("text-fuchsia-100"))
                                        .with(format!("🧵 {parent_name}")),
                                    input([
                                        name("create_parents[]"),
                                        type_("hidden"),
                                        value(parent_name),
                                    ]),
                                ])
                            })
                            .collect::<Vec<_>>(),
                    ),
                ]),
                search(data),
                errors.view("root"),
                // TODO Refresh whole page in case there's a new list in the sidebar
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

fn search(
    Data {
        errors,
        input: input_data,
        search_results,
        ..
    }: &Data,
) -> Element {
    fragment().with([
        errors.view("list_search_term"),
        div(class("relative")).with([
            input([
                class("rounded py-1.5 px-3 my-2 bg-neutral-900 w-full"),
                attr("hx-indicator", "#list_search_term_indicator"),
                attr("hx-post", "/bookmarks/create"),
                attr("hx-select", "#search_results"),
                attr("hx-swap", "outerHTML"),
                attr("hx-target", "#search_results"),
                attr("hx-trigger", "input changed delay:200ms,search"),
                id("list_search_term"),
                name("list_search_term"),
                type_("search"),
                value(input_data.list_search_term.as_deref().unwrap_or_default()),
            ]),
            span(class("absolute flex items-center right-0 top-0 w-0 h-full")).with(span([
                class(
                    "block w-4 h-4 -ml-6 border-2 rounded-full border-neutral-400 animate-spin \
                     border-t-neutral-900 htmx-indicator",
                ),
                id("list_search_term_indicator"),
            ])),
        ]),
        div([class("overflow-y-scroll max-h-96"), id("search_results")]).with([
            if search_results.is_empty() {
                if let Some(term) = &input_data.list_search_term {
                    button([
                        class(
                            "block w-full px-4 pt-1 pb-2 text-left rounded hover:bg-neutral-700 \
                             text-fuchsia-100",
                        ),
                        attr("hx-params", "not list_search_term"),
                        attr("hx-post", "/bookmarks/create"),
                        attr("hx-select", "#selected_lists"),
                        attr("hx-target", "#selected_lists"),
                        name("create_parents[]"),
                        value(term),
                    ])
                    .with(format!(r#"Create public list "{term}""#))
                } else {
                    nothing()
                }
            } else {
                fragment().with(
                    search_results
                        .iter()
                        .map(|list| {
                            button([
                                class(
                                    "block w-full px-4 pt-1 pb-2 text-left rounded \
                                     hover:bg-neutral-700 text-fuchsia-100",
                                ),
                                attr("hx-params", "not list_search_term"),
                                attr("hx-post", "/bookmarks/create"),
                                attr("hx-select", "#selected_lists"),
                                attr("hx-target", "#selected_lists"),
                                name("parents[]"),
                                value(list.id),
                            ])
                            .with(format!("🧵 {}", list.title))
                        })
                        .collect::<Vec<_>>(),
                )
            },
        ]),
    ])
}
