use htmf::prelude_inline::*;
use uuid::Uuid;

use crate::{
    db,
    views::{content, layout},
};

pub struct Data {
    pub layout: layout::Template,
    pub bookmark: db::Bookmark,
    pub archive: Option<db::Archive>,
    pub backlinks: Vec<db::List>,
    pub username: String,
}

pub fn view(
    Data {
        layout,
        bookmark,
        archive,
        backlinks,
        username,
    }: Data,
) -> Element {
    let is_owner = layout
        .authed_info
        .as_ref()
        .is_some_and(|info| info.ap_user_id == bookmark.ap_user_id);

    layout::layout(
        fragment([
            header(
                class("bg-neutral-900 px-4 pt-3 pb-4"),
                [
                    h1(class("text-3xl tracking-tight font-bold"), &bookmark.title),
                    p(
                        class(
                            "w-full overflow-hidden hover:text-fuchsia-300 whitespace-nowrap \
                             text-ellipsis text-neutral-300",
                        ),
                        a(
                            href(&bookmark.url),
                            [
                                span(class("text-neutral-400 text-sm"), "↪"),
                                text(" "),
                                span((), &bookmark.url),
                            ],
                        ),
                    ),
                    status(&bookmark, archive.as_ref(), &username),
                    archive_button(bookmark.id, "Re-archive", archive.as_ref(), is_owner),
                    backlink_section(&backlinks),
                ],
            ),
            div(class("border-b border-black"), ()),
            div(class("border-b border-neutral-700"), ()),
            div(
                id("archive-contents"),
                archive_contents(archive.as_ref(), bookmark.id, is_owner),
            ),
        ]),
        &layout,
    )
}

fn status(bookmark: &db::Bookmark, archive: Option<&db::Archive>, username: &str) -> Element {
    let archive_status = archive.map(|archive| match archive.status {
        db::archives::Status::Success => {
            format!("archived on {}", content::format_date(archive.created_at))
        }
        db::archives::Status::Error => format!(
            "last archive attempt on {}",
            content::format_date(archive.created_at)
        ),
        db::archives::Status::Pending => format!(
            "archive requested on {}",
            content::format_date(archive.created_at)
        ),
    });
    let created_at = content::format_date(bookmark.created_at);

    div(
        class("flex flex-wrap text-sm gap-x-1 text-neutral-400 mb-4"),
        [
            p((), format!("bookmarked by {username} on {created_at}")),
            archive_status.map_or(nothing(), |status| {
                fragment([text(content::BULLET), p((), status)])
            }),
        ],
    )
}

fn archive_button(
    bookmark_id: Uuid,
    label: &str,
    archive: Option<&db::Archive>,
    is_owner: bool,
) -> Element {
    if !is_owner || archive.is_some_and(|a| a.status == db::archives::Status::Pending) {
        return nothing();
    }

    form(
        [
            action(format!("/bookmarks/{bookmark_id}/archive")),
            method("post"),
        ],
        button(
            class(
                "text-sm text-neutral-400 hover:bg-neutral-700 border rounded border-neutral-700 \
                 py-2 px-4",
            ),
            label,
        ),
    )
}

fn archive_contents(archive: Option<&db::Archive>, bookmark_id: Uuid, is_owner: bool) -> Element {
    let Some(archive) = archive else {
        return div(
            class("p-4 flex flex-col gap-2"),
            [
                p(
                    class("text-neutral-500 italic text-sm"),
                    "Not archived yet.",
                ),
                archive_button(bookmark_id, "Archive now", archive, is_owner),
            ],
        );
    };

    if matches!(archive.status, db::archives::Status::Pending) {
        return div(
            [
                attr("hx-get", format!("/bookmarks/{bookmark_id}")),
                attr("hx-trigger", "load delay:1s"),
                attr("hx-select", "#archive-contents"),
                attr("hx-target", "#archive-contents"),
                attr("hx-swap", "outerHTML"),
            ],
            [p(
                class("flex items-center gap-x-1 p-4"),
                [
                    span(
                        class(
                            "inline-block w-4 h-4 border-2 rounded-full border-b-neutral-300 \
                             border-r-neutral-300 border-l-neutral-300 border-t-transparent  \
                             animate-spin",
                        ),
                        (),
                    ),
                    text("Archiving..."),
                ],
            )],
        );
    }

    let Some(html) = &archive.extracted_html else {
        let error = archive
            .error
            .as_ref()
            .map_or("An unknown error occurred.".to_string(), |e| {
                e.0.to_string()
            });
        return div(
            class("p-4 flex flex-col gap-2"),
            [
                p(
                    class("text-orange-300 text-sm italic"),
                    format!("Could not archive this page: {error}"),
                ),
                archive_button(bookmark_id, "Retry archiving", Some(archive), is_owner),
            ],
        );
    };

    div(class("prose prose-invert px-4"), unsafe_raw_html(html))
}

fn backlink_section(backlinks: &[db::List]) -> Element {
    if backlinks.is_empty() {
        return nothing();
    }

    let link_elems = itertools::intersperse(
        backlinks.iter().map(|list| {
            fragment(a(
                [
                    href(format!("/lists/{}", list.id)),
                    class("hover:text-fuchsia-300"),
                ],
                &list.title,
            ))
        }),
        span((), " ∙ "),
    )
    .collect::<Vec<_>>();

    section(
        class("mt-4"),
        [
            h2(
                class("font-bold mb-0.5 text-sm tracking-tight flex gap-1"),
                [
                    span((), "Backlinks"),
                    span(
                        [
                            title_attr("Backlinks are lists that point to this bookmark."),
                            class("text-neutral-400 hover:text-neutral-200 cursor-default text-sm"),
                        ],
                        "🛈",
                    ),
                ],
            ),
            p((), link_elems),
        ],
    )
}
