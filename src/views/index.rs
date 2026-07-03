use htmf::{attr::Attrs, prelude_inline::*};
use url::Url;
use uuid::Uuid;

use super::layout;
use crate::{
    built_version,
    db::{AppTx, layout::AuthedInfo},
    response_error::ResponseResult,
    views::content::BULLET,
};

pub struct Data<'a> {
    pub layout: &'a layout::Template,
    pub base_url: &'a Url,
    pub authed_info: &'a AuthedInfo,
}

struct UserStats {
    bookmark_count: i64,
    list_count: i64,
}

// This is a little experiment: usually, we keep queries in the db module. But
// since this data is only used here, let's see how well it works to keep
// queries close to where their output is used.
async fn user_stats(tx: &mut AppTx, ap_user_id: Uuid) -> ResponseResult<UserStats> {
    let bookmarks = sqlx::query!(
        r#"
        select count(bookmarks) as "count!"
        from bookmarks
        where bookmarks.ap_user_id = $1
        "#,
        ap_user_id
    )
    .fetch_one(&mut **tx)
    .await?;
    let lists = sqlx::query!(
        r#"
        select count(lists) as "count!"
        from lists
        where lists.ap_user_id = $1
        "#,
        ap_user_id
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(UserStats {
        bookmark_count: bookmarks.count,
        list_count: lists.count,
    })
}

pub async fn view(data: &Data<'_>, tx: &mut AppTx) -> ResponseResult<Element> {
    let user_stats = user_stats(tx, data.authed_info.ap_user_id).await?;
    let element = super::layout::layout(
        [
            div(class("border-t dark:border-black border-neutral-200"), ()),
            div(
                class("border-t dark:border-neutral-700 border-neutral-300"),
                (),
            ),
            div(
                class("px-4 flex flex-col w-full items-center text-center"),
                [
                    header(
                        class("mt-12 mx-4 mb-6"),
                        [
                            h1(
                                class("text-2xl font-bold flex items-center gap-2"),
                                [
                                    img([src("/assets/logo_icon_only.png"), class("inline h-8")]),
                                    span(
                                        (),
                                        format!("Welcome to ties, {}!", data.authed_info.username),
                                    ),
                                ],
                            ),
                            p(
                                class("mt-2 dark:text-neutral-400 text-neutral-500"),
                                [
                                    span((), "You have "),
                                    span(
                                        class("dark:text-neutral-300 text-neutral-600"),
                                        user_stats.bookmark_count.to_string(),
                                    ),
                                    span((), " bookmarks  in "),
                                    span(
                                        class("dark:text-neutral-300 text-neutral-600"),
                                        user_stats.list_count.to_string(),
                                    ),
                                    span((), " lists."),
                                ],
                            ),
                        ],
                    ),
                    // TODO add intro text: what can you do with ties? How to get started?  Where
                    // to get help?
                    div(
                        class("flex gap-2 flex-wrap flex-auto mb-2 w-full max-w-lg"),
                        [
                            dash_button(href("/bookmarks/create"), "Add a bookmark"),
                            dash_button(href("/lists/create"), "Create a list"),
                        ],
                    ),
                    div(
                        [class("flex gap-2 flex-wrap flex-auto w-full max-w-lg")],
                        [
                            dash_button_small(
                                href(format!("/user/{}", data.authed_info.username)),
                                "View my profile",
                            ),

                            form([action("/invites/create"), method("post")], button(
                                [
                                    class(
                                        "block px-8 py-2 basis-sm grow border rounded dark:border-neutral-700 border-neutral-300 dark:hover:bg-neutral-700 hover:bg-neutral-100",
                                    ),
                                ],
                                "Invite a user")),
                            form(
                                [action("/logout"), method("post"), class("basis-sm grow")],
                                button(
                                    class(
                                        "block w-full px-4 py-2 border rounded
                                        dark:border-neutral-700 border-neutral-300                                              dark:hover:bg-neutral-700 hover:bg-neutral-100",
                                    ),
                                    "Logout",
                                ),
                            ),
                        ],
                    ),
                    bookmarklet_section(data),
                    // TODO add social links here
                    bottom_info(),
                ],
            ),
        ],
        data.layout,
    );
    Ok(element)
}

fn dash_button<C: Into<Element>>(attrs: Attrs, children: C) -> Element {
    a(
        [
            class(
                "block px-8 py-4 basis-sm grow border rounded dark:border-neutral-700 \
                 border-neutral-300 dark:hover:bg-neutral-700 hover:bg-neutral-100",
            ),
            attrs,
        ],
        children,
    )
}

fn dash_button_small<C: Into<Element>>(attrs: Attrs, children: C) -> Element {
    a(
        [
            class(
                "block px-8 py-2 basis-sm grow border rounded dark:border-neutral-700 \
                 border-neutral-300 dark:hover:bg-neutral-700 hover:bg-neutral-100",
            ),
            attrs,
        ],
        children,
    )
}

pub fn bookmarklet_section(data: &Data) -> Element {
    fragment([
        header(
            class("mt-16"),
            [h2(class("font-bold"), "Install Bookmarklet")],
        ),
        section(
            class("pt-2 pb-4"),
            [bookmarklet_help(), bookmarklet(data.base_url)],
        ),
    ])
}

fn bookmarklet_help() -> Element {
    fragment([
        p(
            class("mb-2"),
            "Click the bookmarklet on any website to add it as a bookmark in
      ties!",
        ),
        p(
            [class("mb-4")],
            "To install, drag the following link to your bookmarks / favorites toolbar:",
        ),
    ])
}

fn bookmarklet(base_url: &Url) -> Element {
    // window.open(
    //   "{ base_url }bookmarks/create?url="
    //   +encodeURIComponent(window.location.href)
    //   +"&title="
    //   +encodeURIComponent(document.title)
    // )
    a(
        [
            class(
                "text-center block my-2 font-bold dark:text-orange-200 text-orange-800 border \
                 rounded py-2 px-16 cursor-grab",
            ),
            href(format!(
                "javascript:(function()%7Bwindow.open(%0A%20%20%22{base_url}bookmarks%2Fcreate%\
                 3Furl%3D%22%0A%20%20%2BencodeURIComponent(window.location.href)%0A%20%20%2B%22%\
                 26title%3D%22%0A%20%20%2BencodeURIComponent(document.title)%0A)%7D)()",
            )),
        ],
        "Add to ties",
    )
}

fn bottom_info() -> Element {
    p(
        class("text-sm flex gap-x-1 p-4 dark:text-neutral-400 text-neutral-500 mt-8"),
        [
            span((), format!("ties {}", built_version::describe_version())),
            span((), BULLET),
            a(
                [
                    href("https://github.com/raffomania/ties"),
                    class("underline"),
                ],
                "Source",
            ),
        ],
    )
}
