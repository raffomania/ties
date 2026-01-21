use anyhow::{Context, anyhow};
use serde::Deserialize;
use sqlx::{FromRow, query, query_as};
use time::OffsetDateTime;
use uuid::Uuid;

use super::AppTx;
use crate::{db, forms::links::CreateLink, response_error::ResponseResult};

#[derive(FromRow, Debug)]
#[expect(dead_code)]
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
pub enum LinkDestinationWithChildren {
    Bookmark(db::Bookmark),
    List(db::ListWithLinks),
}

impl LinkDestinationWithChildren {
    pub fn id(&self) -> Uuid {
        match self {
            LinkDestinationWithChildren::Bookmark(b) => b.id,
            LinkDestinationWithChildren::List(l) => l.list.id,
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
            LinkDestination::Bookmark(b) => b.path(),
            LinkDestination::List(n) => n.path(),
        }
    }
}

pub struct LinkWithContent {
    pub id: Uuid,
    #[expect(dead_code)]
    pub created_at: OffsetDateTime,
    #[expect(dead_code)]
    pub user_id: Uuid,

    pub dest: LinkDestinationWithChildren,
}

// TODO: when showing backlinks in the browser, this needs to be re-evaluated
// https://github.com/raffomania/ties/issues/147
async fn validate_private_lists_belong_to_same_owner(
    tx: &mut AppTx,
    create_link: &CreateLink,
) -> ResponseResult<()> {
    let dest_is_bookmark = query!(
        r#"
        select exists (
            select 1
            from bookmarks
            where id = $1
        )
        "#,
        create_link.dest
    )
    .fetch_one(&mut **tx)
    .await?;
    if let Some(true) = dest_is_bookmark.exists {
        return Ok(());
    }

    let lists = query!(
        r#"
        select src.ap_user_id as src_ap_user_id,
            src.private as src_private,
            dest.ap_user_id as dest_ap_user_id,
            dest.private as dest_private
        from lists src
        inner join lists dest on dest.id = $2
        where src.id = $1
        "#,
        create_link.src,
        create_link.dest
    )
    .fetch_one(&mut **tx)
    .await
    .context("Failed getting data for authorization check")?;

    // If destination list is public, everything's fine
    if !lists.dest_private {
        return Ok(());
    }

    // If source is public, but destination is private, that's not ok
    if !lists.src_private && lists.dest_private {
        return Err(anyhow!("Can't link from a public list to a private list").into());
    }

    // If source and destination are private, and they belong to the same user, it's
    // ok
    if lists.src_private && lists.dest_private && lists.src_ap_user_id == lists.dest_ap_user_id {
        return Ok(());
    }

    Err(anyhow!("Private lists need to belong to the same owner to be linked").into())
}

pub async fn insert(
    tx: &mut AppTx,
    user_id: Uuid,
    create_link: CreateLink,
) -> ResponseResult<Link> {
    validate_private_lists_belong_to_same_owner(tx, &create_link).await?;

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
            links.created_at as link_created_at,
            links.user_id as link_user_id,

            case when lists.id is not null then
                jsonb_build_object(
                    'list', to_jsonb(lists.*),
                    'links',
                    coalesce(
                        jsonb_agg(lists_bookmarks.*)
                        filter (where lists_bookmarks.id is not null),
                    jsonb_build_array())
                    || coalesce(
                        jsonb_agg(lists_lists.*)
                        filter (where lists_lists.id is not null),
                    jsonb_build_array())
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
            let dest: LinkDestinationWithChildren = serde_json::from_value(row.dest.into())?;
            Ok(LinkWithContent {
                id: row.link_id,
                created_at: row.link_created_at,
                user_id: row.link_user_id,
                dest,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(results)
}

pub async fn delete_by_id(tx: &mut AppTx, id: Uuid) -> ResponseResult<Link> {
    let link = query_as!(
        Link,
        r#"
        delete from links
        where id = $1
        returning *
        "#,
        id
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(link)
}
