use anyhow::Context;
use serde::Deserialize;
use sqlx::{FromRow, query, query_as};
use time::OffsetDateTime;
use uuid::Uuid;

use super::AppTx;
use crate::{
    db,
    forms::links::CreateLink,
    response_error::{ResponseError, ResponseResult},
};

#[derive(FromRow, Debug)]
#[expect(dead_code, reason = "Kept for reference on the DB schema")]
pub struct Link {
    pub id: Uuid,
    pub created_at: OffsetDateTime,
    pub user_id: Uuid,

    pub src_list_id: Option<Uuid>,

    pub dest_bookmark_id: Option<Uuid>,
    pub dest_list_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum LinkDestinationWithMetadata {
    Bookmark(db::Bookmark),
    List(db::ListWithMetadata),
}

impl LinkDestinationWithMetadata {
    pub fn id(&self) -> Uuid {
        match self {
            LinkDestinationWithMetadata::Bookmark(b) => b.id,
            LinkDestinationWithMetadata::List(l) => l.list.id,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum LinkDestination {
    Bookmark(db::Bookmark),
    List(db::List),
}

impl LinkDestination {
    pub fn id(&self) -> Uuid {
        match self {
            LinkDestination::Bookmark(b) => b.id,
            LinkDestination::List(n) => n.id,
        }
    }

    pub fn path(&self) -> String {
        match self {
            LinkDestination::Bookmark(b) => b.show_path(),
            LinkDestination::List(n) => n.path(),
        }
    }
}

pub struct LinkWithContent {
    pub id: Uuid,

    pub dest: LinkDestinationWithMetadata,
}

/// Validate that the link source belongs to the user creating the new link.
fn validate_src_belongs_to_creator(src: &db::List, ap_user_id: Uuid) -> ResponseResult<()> {
    if src.ap_user_id != ap_user_id {
        return Err(ResponseError::NotFound);
    }

    Ok(())
}

/// Validate that private link sources or targets can only be involved in links
/// by their owner.
async fn validate_private_items_belong_to_creator(
    tx: &mut AppTx,
    src: &db::List,
    dest: &LinkDestination,
    ap_user_id: Uuid,
) -> ResponseResult<()> {
    if src.private && src.ap_user_id != ap_user_id {
        return Err(ResponseError::NotFound);
    }

    match dest {
        LinkDestination::Bookmark(bookmark) => {
            if !db::bookmarks::is_public_or_owner(tx, ap_user_id, bookmark.id).await? {
                return Err(ResponseError::NotFound);
            }
        }
        LinkDestination::List(list) => {
            if list.private && list.ap_user_id != ap_user_id {
                return Err(ResponseError::NotFound);
            }
        }
    }

    Ok(())
}

pub async fn insert(
    tx: &mut AppTx,
    user_id: Uuid,
    ap_user_id: Uuid,
    create_link: CreateLink,
) -> ResponseResult<Link> {
    let src_list = db::lists::by_id(tx, create_link.src).await?;
    let dest_item = db::items::by_id(tx, create_link.dest).await?;
    validate_src_belongs_to_creator(&src_list, ap_user_id)?;
    validate_private_items_belong_to_creator(tx, &src_list, &dest_item, ap_user_id).await?;

    let list = query_as!(
        Link,
        r#"
        insert into links
        (
            user_id,
            src_list_id,
            dest_bookmark_id,
            dest_list_id
        )
        values ($1,
            (select id from lists where id = $2),
            (select id from bookmarks where id = $3),
            (select id from lists where id = $3)
        )
        returning *"#,
        user_id,
        create_link.src,
        create_link.dest
    )
    .fetch_one(&mut **tx)
    .await
    .context("Failed inserting link")?;

    Ok(list)
}

pub async fn list_by_list(
    tx: &mut AppTx,
    list_id: Uuid,
    ap_user_id: Option<Uuid>,
) -> ResponseResult<Vec<LinkWithContent>> {
    let rows = query!(
        r#"
        select
            links.id as link_id,

            case when lists.id is not null then
                jsonb_build_object(
                    'list', to_jsonb(lists.*),
                    'metadata', jsonb_build_object(
                        'linked_bookmark_count', count(lists_bookmarks.*)
                            filter (where lists_bookmarks.id is not null),
                        'linked_list_count', count(lists_lists.*)
                            filter (where lists_lists.id is not null),
                        'username', (select username from users where users.ap_user_id = lists.ap_user_id)
                    )
                )
            when bookmarks.id is not null then
                to_jsonb(bookmarks.*)
            else null end as dest
        from links

        left join lists on lists.id = links.dest_list_id
        left join links as lists_links on lists_links.src_list_id = lists.id
        left join bookmarks as lists_bookmarks on lists_bookmarks.id = lists_links.dest_bookmark_id
        left join lists as lists_lists on lists_lists.id = lists_links.dest_list_id

        left join bookmarks on bookmarks.id = links.dest_bookmark_id

        where links.src_list_id = $1
            and (lists is null or not lists.private or lists.ap_user_id = $2)
            and (lists_lists is null or not lists_lists.private or lists.ap_user_id = $2)
        group by links.id, lists.id, bookmarks.id
        order by links.created_at desc
        "#,
        list_id,
        ap_user_id
    )
    .fetch_all(&mut **tx)
    .await?;

    let results = rows
        .into_iter()
        .map(|row| {
            let dest: LinkDestinationWithMetadata = serde_json::from_value(row.dest.into())?;
            Ok(LinkWithContent {
                id: row.link_id,
                dest,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(results)
}

pub async fn delete_by_id(tx: &mut AppTx, id: Uuid, user_id: Uuid) -> ResponseResult<Link> {
    let link = query_as!(
        Link,
        r#"
        delete from links
        where id = $1 and user_id = $2
        returning *
        "#,
        id,
        user_id
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(link)
}
