use serde::Deserialize;
use sqlx::{FromRow, query, query_as};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{AppTx, LinkDestination};
use crate::{forms::lists::CreateList, response_error::ResponseResult};

#[derive(FromRow, Debug, Deserialize, Clone)]
pub struct List {
    pub id: Uuid,
    #[serde(with = "time::serde::iso8601")]
    #[expect(dead_code)]
    pub created_at: OffsetDateTime,
    pub ap_user_id: Uuid,

    pub title: String,
    pub content: Option<String>,
    pub private: bool,
    pub pinned: bool,
}

#[derive(FromRow, Debug, Deserialize, Clone)]
pub struct Metadata {
    pub linked_bookmark_count: i64,
    pub linked_list_count: i64,
    pub username: String,
}

impl List {
    pub fn path(&self) -> String {
        let id = self.id;
        format!("/lists/{id}")
    }
}

#[derive(Deserialize)]
pub struct ListWithLinks {
    pub list: List,

    #[serde(deserialize_with = "serde_aux::field_attributes::deserialize_default_from_null")]
    pub links: Vec<LinkDestination>,
}

pub struct ListWithMetadata {
    pub list: List,
    pub metadata: Metadata,
}

pub async fn insert(
    tx: &mut AppTx,
    ap_user_id: Uuid,
    create_list: CreateList,
) -> ResponseResult<List> {
    let list = query_as!(
        List,
        r#"
        insert into lists
        (ap_user_id, title, content, private)
        values ($1, $2, $3, $4)
        returning *"#,
        ap_user_id,
        create_list.title,
        create_list.content,
        create_list.private,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(list)
}
pub async fn edit_title(tx: &mut AppTx, list_id: Uuid, new_title: String) -> ResponseResult<()> {
    query!(
        r#"
        update lists
        set title = $1
        where id = $2"#,
        new_title,
        list_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn by_id(tx: &mut AppTx, list_id: Uuid) -> ResponseResult<List> {
    let list = query_as!(
        List,
        r#"
        select * from lists
        where id = $1
        "#,
        list_id,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(list)
}

pub async fn pointing_to_list(
    tx: &mut AppTx,
    list_id: Uuid,
    ap_user_id: Option<Uuid>,
) -> ResponseResult<Vec<List>> {
    let list = query_as!(
        List,
        r#"
        select * from lists
        where exists (
            select null from links
            where links.dest_list_id = $1
            and links.src_list_id = lists.id
        )
        and (not lists.private or lists.ap_user_id = $2)
        "#,
        list_id,
        ap_user_id
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(list)
}

pub async fn pointing_to_bookmark(
    tx: &mut AppTx,
    bookmark_id: Uuid,
    ap_user_id: Option<Uuid>,
) -> ResponseResult<Vec<List>> {
    let list = query_as!(
        List,
        r#"
        select * from lists
        where exists (
            select null from links
            where links.dest_bookmark_id = $1
            and links.src_list_id = lists.id
        )
        and (not lists.private or lists.ap_user_id = $2)
        "#,
        bookmark_id,
        ap_user_id
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(list)
}

pub async fn metadata_by_id(tx: &mut AppTx, list_id: Uuid) -> ResponseResult<Metadata> {
    let list = query_as!(
        Metadata,
        r#"
            select
                users.username,
                count(links.dest_bookmark_id) as "linked_bookmark_count!",
                count(links.dest_list_id) as "linked_list_count!"
            from lists
            join users on lists.ap_user_id = users.ap_user_id
            left join links
                on lists.id = links.src_list_id
            where lists.id = $1
            group by users.username, users.email
        "#,
        list_id,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(list)
}

pub async fn list_by_id(tx: &mut AppTx, list_ids: &[Uuid]) -> ResponseResult<Vec<List>> {
    let list = query_as!(
        List,
        r#"
        select * from lists
        where id = any($1)
        "#,
        list_ids,
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(list)
}

pub async fn list_pinned_by_user(tx: &mut AppTx, ap_user_id: Uuid) -> ResponseResult<Vec<List>> {
    let lists = query_as!(
        List,
        r#"
        select * from lists
        where ap_user_id = $1 and pinned
        "#,
        ap_user_id,
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(lists)
}

pub async fn list_public_by_user(
    tx: &mut AppTx,
    ap_user_id: Uuid,
) -> ResponseResult<Vec<ListWithMetadata>> {
    let lists = query!(
        r#"
        select lists.* as l,
            ap_users.username,
            count(links.dest_bookmark_id) as "linked_bookmark_count!",
            count(links.dest_list_id) as "linked_list_count!"
        from lists

        join ap_users on lists.ap_user_id = ap_users.id
        left join links
            on lists.id = links.src_list_id
        where lists.ap_user_id = $1 and not private
        group by lists.id, ap_users.username
        "#,
        ap_user_id,
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|record| ListWithMetadata {
        list: List {
            id: record.id,
            created_at: record.created_at,
            ap_user_id: record.ap_user_id,
            title: record.title,
            content: record.content,
            private: record.private,
            pinned: record.pinned,
        },
        metadata: Metadata {
            linked_bookmark_count: record.linked_bookmark_count,
            linked_list_count: record.linked_list_count,
            username: record.username,
        },
    })
    .collect();

    Ok(lists)
}

pub async fn search(tx: &mut AppTx, term: &str, ap_user_id: Uuid) -> ResponseResult<Vec<List>> {
    let lists = query_as!(
        List,
        r#"
            select *
            from lists
            where (lists.title ilike '%' || $1 || '%')
            and lists.ap_user_id = $2
            limit 10
        "#,
        term,
        ap_user_id,
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(lists)
}

pub async fn list_recent(tx: &mut AppTx, ap_user_id: Uuid) -> ResponseResult<Vec<List>> {
    let lists = query_as!(
        List,
        r#"
            select lists.*
            from lists
            left join links as src_links on lists.id = src_links.src_list_id
            left join links as dest_links on lists.id = dest_links.dest_list_id
            where lists.ap_user_id = $1
            group by lists.id
            order by
                max(src_links.created_at) desc nulls last,
                max(dest_links.created_at) nulls last,
                max(lists.created_at) desc
            limit 500
        "#,
        ap_user_id,
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(lists)
}

pub struct UnpinnedList {
    pub id: Uuid,

    pub title: String,
    pub content: Option<String>,
    pub bookmark_count: i64,
    pub linked_list_count: i64,
}

pub async fn list_unpinned(tx: &mut AppTx, ap_user_id: Uuid) -> ResponseResult<Vec<UnpinnedList>> {
    let lists = query_as!(
        UnpinnedList,
        r#"
            select lists.id, title, content,
                count(links.dest_bookmark_id) as "bookmark_count!",
                count(links.dest_list_id) as "linked_list_count!"
            from lists
            left join links
                on lists.id = links.src_list_id
            where lists.ap_user_id = $1 and not pinned
            group by lists.id
        "#,
        ap_user_id,
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(lists)
}

pub async fn set_private(tx: &mut AppTx, list_id: Uuid, private: bool) -> ResponseResult<List> {
    let list = query_as!(
        List,
        r#"
        update lists
        set private = $1
        where id = $2
        returning *
        "#,
        private,
        list_id,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(list)
}

pub async fn set_pinned(tx: &mut AppTx, list_id: Uuid, pinned: bool) -> ResponseResult<List> {
    let list = query_as!(
        List,
        r#"
        update lists
        set pinned = $1
        where id = $2
        returning *
        "#,
        pinned,
        list_id,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(list)
}
