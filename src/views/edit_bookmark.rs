use htmf::{into_attrs::IntoAttrs, prelude_inline::*};
use sqlx::query_as;
use uuid::Uuid;

use super::layout;
use crate::{
    authentication::AuthUser,
    db::{self, AppTx},
    form_errors::FormErrors,
    forms::{self, bookmarks::EditQuery},
    response_error::ResponseResult,
    views::content::help_icon,
};

pub struct Loaded {
    pub layout: layout::Template,
    pub bookmark: db::Bookmark,
    pub connected_lists: Vec<LinkWithList>,
    pub title_from_archive: Option<String>,
    pub query: forms::bookmarks::EditQuery,
}

pub async fn load(
    tx: &mut AppTx,
    auth_user: &AuthUser,
    bookmark_id: Uuid,
    query: forms::bookmarks::EditQuery,
) -> ResponseResult<Loaded> {
    let layout = layout::Template::from_db(tx, Some(auth_user)).await?;
    let bookmark = db::bookmarks::by_id(tx, bookmark_id).await?;
    let connected_lists = query_connected_lists(tx, bookmark_id, auth_user.user_id).await?;
    let title_from_archive = query_archive_title(tx, bookmark_id).await?;

    Ok(Loaded {
        layout,
        bookmark,
        connected_lists,
        title_from_archive,
        query,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionOutcome {
    Renamed,
    NoAction,
}

pub struct ViewData {
    pub layout: layout::Template,
    pub bookmark: db::Bookmark,
    pub connected_lists: Vec<LinkWithList>,
    pub search_list_results: Vec<db::List>,
    pub errors: FormErrors,
    pub rename_input: forms::bookmarks::Rename,
    pub search_query: forms::bookmarks::EditQuery,
    pub outcome: ActionOutcome,
    pub title_from_archive: Option<String>,
}

impl From<Loaded> for ViewData {
    fn from(
        Loaded {
            layout,
            bookmark,
            connected_lists,
            title_from_archive,
            query,
        }: Loaded,
    ) -> Self {
        let rename_input = forms::bookmarks::Rename {
            title: bookmark.title.clone(),
        };

        Self {
            layout,
            bookmark,
            connected_lists,
            errors: FormErrors::default(),
            rename_input,
            search_query: query,
            search_list_results: Vec::new(),
            outcome: ActionOutcome::NoAction,
            title_from_archive,
        }
    }
}

impl ViewData {
    pub fn view(&self) -> Element {
        view(self)
    }

    pub async fn load_search_results(
        self,
        tx: &mut AppTx,
        ap_user_id: Uuid,
    ) -> ResponseResult<Self> {
        let search_list_results = search_unconnected_lists(
            tx,
            &self.search_query.search_term,
            ap_user_id,
            self.bookmark.id,
            self.search_query.search_public_lists,
        )
        .await?;

        Ok(Self {
            search_list_results,
            ..self
        })
    }
}

#[derive(Debug)]
pub struct LinkWithList {
    pub link_id: Uuid,
    pub list_title: String,
    pub list_private: bool,
}

async fn query_connected_lists(
    tx: &mut AppTx,
    bookmark_id: Uuid,
    user_id: Uuid,
) -> ResponseResult<Vec<LinkWithList>> {
    let links = query_as!(
        LinkWithList,
        r#"
        select links.id as "link_id", lists.title as "list_title", lists.private as "list_private"
        from links
        join lists on links.src_list_id = lists.id
        where links.dest_bookmark_id = $1
            and links.user_id = $2
        "#,
        bookmark_id,
        user_id
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(links)
}

async fn query_archive_title(tx: &mut AppTx, bookmark_id: Uuid) -> ResponseResult<Option<String>> {
    let row = sqlx::query!(
        r#"
        select extracted_title
        from archives
        where archives.bookmark_id = $1
        "#,
        bookmark_id,
    )
    .fetch_optional(&mut **tx)
    .await?;

    Ok(row.and_then(|r| r.extracted_title))
}

pub async fn search_unconnected_lists(
    tx: &mut AppTx,
    term: &str,
    ap_user_id: Uuid,
    bookmark_id: Uuid,
    search_public_lists: bool,
) -> ResponseResult<Vec<db::List>> {
    let lists = query_as!(
        db::List,
        r#"
            select lists.*
            from lists
            left join links as src_links on lists.id = src_links.src_list_id
            left join links as dest_links on lists.id = dest_links.dest_list_id
            where (lists.title ilike '%' || $1 || '%')
                and lists.ap_user_id = $2
                and ($4 or lists.private)
                and not exists (
                    select 1 from links
                    where links.dest_bookmark_id = $3
                    and links.src_list_id = lists.id
                )
            group by lists.id
            order by
                greatest(max(dest_links.created_at), max(src_links.created_at)) desc nulls last,
                max(lists.created_at) desc
            limit 10
        "#,
        term,
        ap_user_id,
        bookmark_id,
        search_public_lists
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(lists)
}

pub fn view(
    view_data @ ViewData {
        layout,
        errors,
        connected_lists: backlinks,
        bookmark,
        search_list_results,
        search_query,
        ..
    }: &ViewData,
) -> Element {
    let bookmark_private = backlinks.iter().all(|list| list.list_private);

    layout::layout(
        [
            div(class("border-t border-black"), ()),
            div(class("border-t border-neutral-700"), ()),
            div(
                [
                    class("flex flex-col max-w-xl px-4 pb-4 rounded grow bg-neutral-800 mx-auto"),
                    attr("hx-indicator", "#global-spinner"),
                ],
                [
                    a(
                        [
                            href(format!("/bookmarks/{}", bookmark.id)),
                            class(
                                "hover:bg-neutral-700 rounded text-neutral-400 self-start mt-4 \
                                 py-0.5 px-2 -ml-2",
                            ),
                        ],
                        "← back",
                    ),
                    header(
                        class("mb-4 flex justify-between"),
                        [
                            h1(class("text-xl font-bold"), "Edit bookmark"),
                            span(
                                [class("inline-block h-4"), id("global-spinner")],
                                span(
                                    class(
                                        "block w-4 h-4 border-2 rounded-full border-l-neutral-200 \
                                         border-b-neutral-200 border-r-neutral-200 animate-spin \
                                         border-t-transparent htmx-indicator",
                                    ),
                                    (),
                                ),
                            ),
                        ],
                    ),
                    dl(
                        (),
                        [
                            dt(class("text-sm text-neutral-400"), "URL"),
                            dd(
                                class("overflow-hidden whitespace-nowrap text-ellipsis"),
                                &bookmark.url,
                            ),
                            dt(
                                [
                                    class("text-sm text-neutral-400 mt-2"),
                                    title_attr(
                                        "Bookmarks are private until they get added to a public list.",
                                    ),
                                ],
                                [span(class("mr-0.5"), "Visibility"), help_icon()],
                            ),
                            dd(
                                (),
                                if bookmark_private {
                                    "Private"
                                } else {
                                    "Public"
                                },
                            ),
                        ],
                    ),
                    rename(view_data),
                    p(class("mt-6 font-bold"), "Connected lists"),
                    (!backlinks.is_empty())
                        .then(|| disconnect(bookmark, backlinks, search_query))
                        .into(),
                    connect(
                        bookmark,
                        search_list_results,
                        search_query,
                        errors,
                        bookmark_private,
                    ),
                    errors.view("root"),
                ],
            ),
        ],
        layout,
    )
}

fn rename(
    ViewData {
        errors,
        rename_input,
        bookmark,
        outcome,
        title_from_archive,
        search_query: search_input,
        ..
    }: &ViewData,
) -> Element {
    let sq = search_input.query_string();
    form(
        [
            method("POST"),
            action(format!("/bookmarks/{}/rename{}", bookmark.id, sq)),
            attr("hx-boost", "true"),
        ],
        [
            div(
                class("flex justify-between mt-6"),
                [
                    label([for_("rename-title"), class("font-bold")], "Title"),
                    if outcome == &ActionOutcome::Renamed {
                        span([class("text-neutral-300")], "✓ renamed!")
                    } else {
                        nothing()
                    },
                ],
            ),
            errors.view("title"),
            div(
                class("flex flex-wrap gap-2 mt-1 items-center"),
                [
                    input([
                        value(&rename_input.title),
                        class("rounded py-1.5 px-3 bg-neutral-900 grow"),
                        name("title"),
                        id("rename-title"),
                        required(""),
                        type_("text"),
                    ]),
                    button(
                        [
                            class("bg-neutral-300 py-1.5 px-3 text-neutral-900 rounded"),
                            type_("submit"),
                        ],
                        "Rename",
                    ),
                ],
            ),
            title_from_archive
                .as_ref()
                .and_then(|title| {
                    if &bookmark.title == title {
                        return None;
                    }

                    let shortened: String =
                        title.chars().take(title.floor_char_boundary(500)).collect();

                    Some(button(
                        [
                            name("title"),
                            value(&shortened),
                            type_("submit"),
                            class("underline text-sm max-w-full break-all text-left"),
                        ],
                        format!(r#"Use website title: "{shortened}""#),
                    ))
                })
                .into(),
        ],
    )
}

fn disconnect(
    bookmark: &db::Bookmark,
    lists: &[LinkWithList],
    search_input: &EditQuery,
) -> Element {
    let sq = search_input.query_string();
    fragment([form(
        [
            class("flex flex-wrap gap-1 mt-1"),
            id("disconnect"),
            method("POST"),
            action(format!("/bookmarks/{}/disconnect{}", bookmark.id, sq)),
            attr("hx-boost", "true"),
        ],
        lists
            .iter()
            .map(|link| {
                button(
                    [
                        class(
                            "max-w-full border border-neutral-600 rounded px-3 gap-2 flex \
                             items-center hover:bg-neutral-700",
                        ),
                        name("delete_link_id"),
                        value(link.link_id),
                    ],
                    [
                        span([class("text-neutral-400"), title_attr("disconnect")], "✖"),
                        span(
                            class("text-ellipsis whitespace-nowrap overflow-hidden"),
                            &link.list_title,
                        ),
                    ],
                )
            })
            .collect::<Vec<_>>(),
    )])
}

fn connect(
    bookmark: &db::Bookmark,
    search_results: &[db::List],
    search_input: &EditQuery,
    errors: &FormErrors,
    bookmark_private: bool,
) -> Element {
    let edit_url = format!("/bookmarks/{}/edit", bookmark.id);
    let sq = search_input.query_string();
    let connect_action = format!("/bookmarks/{}/connect{}", bookmark.id, sq);
    section(
        [id("connect"), class("mt-2")],
        [
            errors.view("search_term"),
            form(
                [
                    action(&edit_url),
                    method("get"),
                    attr("hx-boost", "true"),
                    attr("hx-push-url", "true"),
                    class("w-full"),
                    attr(
                        "hx-trigger",
                        "input changed delay:.5s from:find #connect-search-term, change from:find \
                         #connect-search-public-lists",
                    ),
                ],
                [
                    div(
                        class("flex gap-2"),
                        [
                            input([
                                type_("search"),
                                name("search_term"),
                                value(&search_input.search_term),
                                placeholder("Search lists to connect..."),
                                // Adding an id will make htmx keep the keyboard focus
                                id("connect-search-term"),
                                class("py-1.5 px-3 bg-neutral-900 grow rounded"),
                            ]),
                            button(
                                [
                                    type_("submit"),
                                    class(
                                        "px-2 text-neutral-400 shrink border rounded \
                                         border-neutral-700",
                                    ),
                                ],
                                "Search lists",
                            ),
                        ],
                    ),
                    div(
                        class("flex items-baseline gap-2 mt-1 mb-2"),
                        [
                            input([type_("hidden"), name("search_public_lists"), value("false")]),
                            input([
                                type_("checkbox"),
                                name("search_public_lists"),
                                value("true"),
                                if search_input.search_public_lists {
                                    checked()
                                } else {
                                    ().into_attrs()
                                },
                                id("connect-search-public-lists"),
                            ]),
                            label(
                                [for_("connect-search-public-lists")],
                                [
                                    p((), "Include public lists in search"),
                                    if bookmark_private && search_input.search_public_lists {
                                        p(
                                            [class("text-neutral-400 text-sm")],
                                            "Connecting this bookmark to a public list will allow \
                                             anyone to see it.",
                                        )
                                    } else {
                                        nothing()
                                    },
                                ],
                            ),
                        ],
                    ),
                ],
            ),
            p(
                class("italic mt-2 text-sm text-neutral-400"),
                if search_results.is_empty() && !search_input.search_term.is_empty() {
                    "Found no lists with a matching title."
                } else if !search_results.is_empty() && search_input.search_term.is_empty() {
                    "Recently used:"
                } else if !search_results.is_empty() {
                    "Search results:"
                } else {
                    ""
                },
            ),
            div(
                class("flex flex-wrap gap-1 mt-1"),
                search_results
                    .iter()
                    .map(|list| {
                        form(
                            [
                                action(&connect_action),
                                method("POST"),
                                attr("hx-boost", "true"),
                                class("contents"),
                            ],
                            [button(
                                [
                                    name("connect_list_id"),
                                    value(list.id),
                                    class(
                                        "max-w-full border border-neutral-600 rounded px-3 gap-2 \
                                         flex items-center hover:bg-neutral-700",
                                    ),
                                ],
                                [
                                    span(
                                        [
                                            title_attr("connect"),
                                            class("font-black text-neutral-400"),
                                        ],
                                        "+",
                                    ),
                                    span(
                                        class("text-ellipsis whitespace-nowrap overflow-hidden"),
                                        &list.title,
                                    ),
                                ],
                            )],
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
        ],
    )
}
