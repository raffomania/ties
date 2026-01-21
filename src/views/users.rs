#[allow(clippy::wildcard_imports)]
use htmf::prelude::*;
use url::Url;

use super::layout::{self, layout};

pub struct ProfileTemplate {
    pub layout: layout::Template,
    pub base_url: Url,
}

pub fn start_page(template: &ProfileTemplate) -> Element {
    layout(
        fragment().with([
            header(class("px-4 pt-3 mb-2"))
                .with([h1(class("text-xl font-bold")).with([text("Install Bookmarklet")])]),
            section(class("p-4")).with([bookmarklet_help(), bookmarklet(&template.base_url)]),
        ]),
        &template.layout,
    )
}

fn bookmarklet_help() -> Element {
    fragment().with([
        p(class("mb-2")).with(
            "Click the bookmarklet on any website to add it as a bookmark in
      ties!",
        ),
        p([]).with("To install, drag the following link to your bookmarks / favorites toolbar:"),
    ])
}

fn bookmarklet(base_url: &Url) -> Element {
    // window.open(
    //   "{ base_url }/bookmarks/create?url="
    //   +encodeURIComponent(window.location.href)
    //   +"&title="
    //   +encodeURIComponent(document.title)
    // )
    a([
        class("block my-2 font-bold text-orange-200"),
        href(format!(
            "javascript:(function()%7Bwindow.open(%0A%20%20%22{base_url}%2Fbookmarks%2Fcreate%\
             3Furl%3D%22%0A%20%20%2BencodeURIComponent(window.location.href)%0A%20%20%2B%22%\
             26title%3D%22%0A%20%20%2BencodeURIComponent(document.title)%0A)%7D)()",
        )),
    ])
    .with("Add to ties")
}
