use htmf::{declare::*, element::Element, into_attrs::IntoAttrs, into_elements::IntoElements};

pub fn base_document(children: impl IntoElements) -> Element {
    document().with(
        html(class("w-full h-full"))
            .with(head([]).with([
                // icons & title
                link([
                    rel("icon"),
                    href("/assets/favicon.svg"),
                    attr("type", "image/svg+xml"),
                ]),
                link([
                    rel("apple-touch-icon"),
                    href("/assets/apple-touch-icon.png"),
                ]),
                title_tag([]).with("ties"),
                // styling
                link([rel("stylesheet"), href("/assets/preflight.css")]),
                link([rel("stylesheet"), href("/assets/railwind.css")]),
                link([rel("stylesheet"), href("/assets/prose.css")]),
                // Hide .js-only elements when JS is disabled.
                noscript([]).with(Element::Tag {
                    tag: "style",
                    attrs: ().into_attrs(),
                    children: ".js-only{display: none;}".into_elements(),
                }),
                // htmx
                script(src("/assets/htmx.1.9.9.js")),
                meta([
                    name("htmx-config"),
                    content(
                        r#"{
                            "scrollIntoViewOnBoost": false,
                            "historyCacheSize": 0
                        }"#,
                    ),
                ]),
                meta([name("color-scheme"), content("dark light")]),
                meta([
                    name("viewport"),
                    content("width=device-width,initial-scale=1"),
                ]),
            ]))
            .with(
                body(class(
                    "w-full h-full dark:text-gray-200 text-gray-700 dark:bg-neutral-800
                     bg-neutral-50",
                ))
                .with(children),
            ),
    )
}
