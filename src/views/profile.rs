use htmf::prelude_inline::*;
use uuid::Uuid;

use crate::{
    db::{self, AppTx},
    response_error::ResponseResult,
    views::{content::pluralize, layout},
};

struct Metadata {
    public_bookmark_count: i64,
}

pub struct Data {
    pub layout: layout::Template,
    pub ap_user: db::ApUser,
    pub public_lists: Vec<db::ListWithMetadata>,
}

async fn get_metadata(tx: &mut AppTx, ap_user_id: Uuid) -> ResponseResult<Metadata> {
    // TODO add indexes to optimize this query.
    // https://github.com/raffomania/ties/issues/153
    Ok(sqlx::query_as!(
        Metadata,
        r#"
            select count(distinct links.dest_bookmark_id) as "public_bookmark_count!"
            from ap_users
            join lists on lists.ap_user_id = ap_users.id
            join links on links.src_list_id = lists.id
            where ap_users.id = $1
                and not lists.private
            "#,
        ap_user_id
    )
    .fetch_one(&mut **tx)
    .await?)
}

pub async fn view(
    mut tx: AppTx,
    Data {
        layout,
        ap_user,
        public_lists: lists,
    }: &Data,
) -> ResponseResult<Element> {
    let metadata = get_metadata(&mut tx, ap_user.id).await?;
    // TODO find out what the user-visible domain of the ApUser is and show it here
    // https://github.com/raffomania/ties/issues/154
    let children = fragment([
        header(
            [class("p-4 bg-neutral-900")],
            [
                h1(
                    [class("text-xl font-bold tracking-tight")],
                    [&ap_user.username],
                ),
                ap_user
                    .bio
                    .as_ref()
                    .map_or(nothing(), |bio| p(class("m-4"), bio)),
            ],
        ),
        view_lists(lists, &metadata),
    ]);

    Ok(layout::layout(children, layout))
}

fn view_lists(lists: &[db::ListWithMetadata], metadata: &Metadata) -> Element {
    section(
        [],
        [p(
            class("px-4 text-neutral-400 pb-3 bg-neutral-900 border-b border-black"),
            [
                span(
                    class("font-bold tracking-tight"),
                    pluralize(
                        lists.len().try_into().unwrap_or(-1),
                        "public list",
                        "public lists",
                    ),
                ),
                span((), " containing "),
                span(
                    (),
                    pluralize(metadata.public_bookmark_count, "bookmark", "bookmarks"),
                ),
            ],
        )],
    )
    .with(lists.iter().map(list_item).collect::<Vec<_>>())
}

fn list_item(list: &db::ListWithMetadata) -> Element {
    section(
        class("px-4 pt-4 pb-4 border-t border-neutral-700"),
        [
            a(
                [
                    class(
                        "block overflow-hidden font-semibold leading-8 hover:text-fuchsia-300 \
                         text-ellipsis whitespace-nowrap",
                    ),
                    href(list.list.path()),
                ],
                &list.list.title,
            ),
            div(
                class("text-sm text-neutral-400 flex flex-wrap gap-x-1"),
                [
                    p(
                        [],
                        pluralize(list.metadata.linked_bookmark_count, "bookmark", "bookmarks"),
                    ),
                    text("∙"),
                    p(
                        [],
                        pluralize(list.metadata.linked_list_count, "list", "lists"),
                    ),
                ],
            ),
        ],
    )
}
