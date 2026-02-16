use activitypub_federation::fetch::object_id::ObjectId;
use serde::Deserialize;
use sqlx::{FromRow, query, query_as};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use super::AppTx;
use crate::{
    db,
    response_error::{ResponseError, ResponseResult},
};

#[derive(FromRow, Debug, Deserialize, Clone)]
pub struct Bookmark {
    pub id: Uuid,
    #[serde(with = "time::serde::iso8601")]
    pub created_at: OffsetDateTime,
    pub ap_user_id: Uuid,

    pub url: String,
    pub title: String,
    pub ap_id: ObjectId<Bookmark>,
}

#[derive(FromRow, Debug)]
struct BookmarkRow {
    id: Uuid,
    created_at: OffsetDateTime,
    ap_user_id: Uuid,

    url: String,
    title: String,
    ap_id: String,
}

impl TryFrom<BookmarkRow> for Bookmark {
    type Error = ResponseError;

    fn try_from(value: BookmarkRow) -> Result<Self, Self::Error> {
        Ok(Bookmark {
            id: value.id,
            created_at: value.created_at,
            ap_user_id: value.ap_user_id,
            url: value.url,
            title: value.title,
            ap_id: value.ap_id.parse()?,
        })
    }
}

impl Bookmark {
    pub fn path(&self) -> String {
        let id = self.id;
        format!("/bookmarks/{id}")
    }
}

pub struct InsertBookmark {
    pub url: String,
    pub title: String,
}

pub async fn insert_local(
    tx: &mut AppTx,
    ap_user_id: Uuid,
    create_bookmark: InsertBookmark,
    base_url: &Url,
) -> ResponseResult<Bookmark> {
    let id = Uuid::new_v4();
    let ap_id = base_url.join("/ap/bookmark/")?.join(&id.to_string())?;
    let bookmark = query_as!(
        BookmarkRow,
        r#"
        insert into bookmarks
        (id, ap_user_id, url, title, ap_id)
        values ($1, $2, $3, $4, $5)
        returning *"#,
        id,
        ap_user_id,
        create_bookmark.url,
        create_bookmark.title,
        ap_id.to_string()
    )
    .fetch_one(&mut **tx)
    .await?;

    bookmark.try_into()
}

pub async fn by_id(tx: &mut AppTx, id: Uuid) -> ResponseResult<Bookmark> {
    let row = query_as!(
        BookmarkRow,
        r#"
        select *
        from bookmarks
        where id = $1;
        "#,
        id
    )
    .fetch_one(&mut **tx)
    .await?;

    Bookmark::try_from(row)
}

pub async fn by_ap_id(tx: &mut AppTx, ap_id: ObjectId<db::Bookmark>) -> ResponseResult<Bookmark> {
    let row = query_as!(
        BookmarkRow,
        r#"
        select *
        from bookmarks
        where ap_id = $1;
        "#,
        ap_id.inner().as_str(),
    )
    .fetch_one(&mut **tx)
    .await?;

    Bookmark::try_from(row)
}

pub async fn list_unsorted(tx: &mut AppTx, ap_user_id: Uuid) -> ResponseResult<Vec<Bookmark>> {
    let bookmarks = query_as!(
        BookmarkRow,
        r#"
        select *
        from bookmarks
        where ap_user_id = $1
        and not exists (
            select null from links
            where dest_bookmark_id = bookmarks.id
        );
        "#,
        ap_user_id,
    )
    .fetch_all(&mut **tx)
    .await?;

    bookmarks
        .into_iter()
        .map(Bookmark::try_from)
        .collect::<ResponseResult<Vec<_>>>()
}

pub async fn delete_by_id(tx: &mut AppTx, id: Uuid) -> ResponseResult<Bookmark> {
    let bookmark = query_as!(
        BookmarkRow,
        r#"
        delete from bookmarks
        where id = $1
        returning *;
        "#,
        id
    )
    .fetch_one(&mut **tx)
    .await?;

    bookmark.try_into()
}

/// Create a new UUID as primary key.
/// Do not use this for local bookmarks as their AP ID needs to correlate with
/// the primary key's UUID.
pub async fn upsert_remote(
    tx: &mut AppTx,
    ap_user_id: Uuid,
    ap_id: &ObjectId<db::Bookmark>,
    insert: InsertBookmark,
) -> ResponseResult<Bookmark> {
    let id = Uuid::new_v4();
    let user = query_as!(
        BookmarkRow,
        r#"
        insert into bookmarks
        (ap_id, id, ap_user_id, url, title)
        values ($1, $2, $3, $4, $5)
        on conflict(ap_id) do update set
            ap_user_id = $2,
            url = $3,
            title = $4
        returning *
        "#,
        ap_id.inner().as_str(),
        id,
        ap_user_id,
        insert.url,
        insert.title,
    )
    .fetch_one(&mut **tx)
    .await?
    .try_into()?;

    Ok(user)
}

/// Return true if at least one public list points to the given bookmark.
pub async fn is_public(tx: &mut AppTx, bookmark_id: Uuid) -> ResponseResult<bool> {
    let public_destination_count = query!(
        r#"
        select count(lists.id) as "count!"
        from links
        inner join lists on links.src_list_id = lists.id
        where not lists.private
            and links.dest_bookmark_id = $1
        "#,
        bookmark_id
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(public_destination_count.count > 0)
}
