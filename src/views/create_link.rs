use htmf::prelude_inline::*;

use super::content;
use crate::{
    db::{self, LinkDestination},
    form_errors::FormErrors,
    forms::links::PartialCreateLink,
};

pub struct Data {
    pub layout: super::layout::Template,

    pub errors: FormErrors,
    pub form_input: PartialCreateLink,
    pub search_results: Vec<db::List>,
    pub src_from_db: Option<LinkDestination>,
    pub dest_from_db: Option<LinkDestination>,
}

pub fn view(
    Data {
        layout,
        errors,
        form_input,
        search_results,
        src_from_db,
        dest_from_db,
    }: Data,
) -> Element {
    super::layout::layout(
        div(
            class("border-t border-black"),
            [
                div(class("border-t border-neutral-700"), ()),
                form(
                    [
                        action("/links/create"),
                        class("mx-4 mb-4"),
                        attr("hx-post", "/links/create"),
                        attr("hx-select", "form#create_link"),
                        attr("hx-swap", "outerHTML"),
                        attr("hx-target", "form#create_link"),
                        id("create_link"),
                        method("POST"),
                    ],
                    [
                        h1(class("pt-3 pb-4 text-xl font-bold"), "Add to list"),
                        match dest_from_db {
                            Some(dest) => label(
                                class("block mb-4"),
                                [
                                    p(class("mb-1"), "Item to add"),
                                    link_dest(&dest),
                                    input([name("dest"), type_("hidden"), value(dest.id())]),
                                ],
                            ),
                            None => nothing(),
                        },
                        match src_from_db {
                            Some(src) => label(
                                class("block mb-2"),
                                [
                                    p(class("mb-1"), "Adding to list"),
                                    link_dest(&src),
                                    input([name("src"), type_("hidden"), value(src.id())]),
                                ],
                            ),
                            None => nothing(),
                        },
                        if form_input.src.is_none() || form_input.dest.is_none() {
                            let (suffix, search_term, label_desc) = if form_input.src.is_none() {
                                ("src", form_input.search_term_src, "Adding to list")
                            } else {
                                ("dest", form_input.search_term_dest, "Item to add")
                            };
                            label(
                                class("block my-2"),
                                [
                                    p(class("mb-1"), label_desc),
                                    errors.view("search_term_src"),
                                    errors.view("search_term_dest"),
                                    input([
                                        class("rounded py-1.5 px-3 bg-neutral-900 w-full"),
                                        attr("hx-post", "/links/create"),
                                        attr("hx-select", "#search_results"),
                                        attr("hx-target", "#search_results"),
                                        attr("hx-trigger", "input changed delay:300ms,search"),
                                        id(format!("search_term_{suffix}")),
                                        name(format!("search_term_{suffix}")),
                                        type_("search"),
                                        value(search_term.unwrap_or_default()),
                                    ]),
                                ],
                            )
                        } else {
                            nothing()
                        },
                        div(
                            [class("overflow-y-scroll max-h-96"), id("search_results")],
                            fragment(
                                search_results
                                    .into_iter()
                                    .map(|list| {
                                        button(
                                            [
                                                class(
                                                    "block w-full px-4 pt-1 pb-2 text-left \
                                                     rounded hover:bg-neutral-700",
                                                ),
                                                attr("hx-post", "/links/create"),
                                                value(list.id),
                                                name(if form_input.src.is_some() {
                                                    "dest"
                                                } else {
                                                    "src"
                                                }),
                                            ],
                                            p(
                                                class("text-fuchsia-100"),
                                                format!("️🧵 {}", list.title),
                                            ),
                                        )
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                        ),
                        errors.view("root"),
                        // TODO creating the link doesn't update the address bar
                        if form_input.src.is_some() && form_input.dest.is_some() {
                            button(
                                [
                                    type_("button"),
                                    name("submitted"),
                                    value("true"),
                                    class(
                                        "bg-neutral-300 py-1.5 px-3 text-neutral-900 rounded mt-4 \
                                         self-end",
                                    ),
                                    // TODO is there a cleaner way to make this form work embedded
                                    // on the lists page?
                                    attr("hx-post", "/links/create"),
                                    attr("hx-select", "main"),
                                    attr("hx-target", "main"),
                                    attr("hx-swap", "innerHTML"),
                                ],
                                "Create Link",
                            )
                        } else {
                            nothing()
                        },
                    ],
                ),
            ],
        ),
        &layout,
    )
}

fn link_dest(link_dest: &db::LinkDestination) -> Element {
    match link_dest {
        db::LinkDestination::Bookmark(bookmark) => fragment([
            p(class("text-orange-100"), format!("📄 {}", bookmark.title)),
            content::link_url(&bookmark.url),
        ]),
        db::LinkDestination::List(list) => {
            p(class("text-fuchsia-100"), format!("️🧵 {}", list.title))
        }
    }
}
