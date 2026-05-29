use anyhow::Context;
use sqlx::types::Json;
use time::{OffsetDateTime, format_description::well_known::Iso8601};
use uuid::Uuid;

use crate::{
    archive,
    db::{self, AppTx},
    response_error::ResponseResult,
};

#[derive(sqlx::Type, Debug, PartialEq, Eq)]
#[sqlx(type_name = "archive_status")]
pub enum Status {
    Success,
    Error,
    Pending,
}

#[derive(sqlx::FromRow, derive_more::Debug)]
#[allow(dead_code, reason = "kept for DB schema reference")]
pub struct Archive {
    pub id: Uuid,

    pub bookmark_id: Uuid,

    pub created_at: OffsetDateTime,
    pub status: Status,
    pub error: Option<Json<archive::Error>>,

    pub extracted_title: Option<String>,
    #[debug("{}", if extracted_html.is_some() { "Some(...)" } else { "None" })]
    pub extracted_html: Option<String>,
    pub byline: Option<String>,
    pub lang: Option<String>,
    pub site_name: Option<String>,
    pub published_time: Option<OffsetDateTime>,
}

pub async fn insert_pending(tx: &mut AppTx, bookmark_id: Uuid) -> ResponseResult<Archive> {
    let id = Uuid::new_v4();
    let status = Status::Pending;

    let archive = sqlx::query_as!(
        Archive,
        r#"
        insert into archives
        (id, bookmark_id, status)
        values ($1, $2, $3)
        returning id, bookmark_id, created_at, status as "status: _", error as "error: Json<archive::Error>", extracted_title, extracted_html, byline, lang, site_name, published_time as "published_time: OffsetDateTime"
        "#,
        id,
        bookmark_id,
        status as Status,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(archive)
}

pub async fn update(
    tx: &mut AppTx,
    archive_id: Uuid,
    article: &Result<legible::Article, archive::Error>,
) -> ResponseResult<Archive> {
    let archive = match article {
        Ok(article) => update_archive_to_success(tx, archive_id, article).await?,
        Err(e) => update_archive_to_error(tx, archive_id, e).await?,
    };

    db::bookmarks::update_search_index(tx, archive.bookmark_id).await?;

    Ok(archive)
}

async fn update_archive_to_success(
    tx: &mut AppTx,
    archive_id: Uuid,
    legible::Article {
        title,
        byline,
        lang,
        content,
        site_name,
        published_time,
        ..
    }: &legible::Article,
) -> ResponseResult<Archive> {
    let published_time = published_time.as_deref().and_then(|s| {
        OffsetDateTime::parse(s, &Iso8601::PARSING)
            .inspect_err(|&e| {
                tracing::warn!(?e, input = s, "Failed to parse published_time as ISO 8601");
            })
            .ok()
    });
    let archive = sqlx::query_as!(
        Archive,
        r#"
            update archives
            set status = $2,
                error = null,
                extracted_html = $3,
                extracted_title = $4,
                byline = $5,
                lang = $6,
                site_name = $7,
                published_time = $8
            where id = $1
            returning id, bookmark_id, created_at, status as "status: _", error as "error: Json<archive::Error>", extracted_title, extracted_html, byline, lang, site_name, published_time as "published_time: OffsetDateTime"
        "#,
        archive_id,
        Status::Success as Status,
        content,
        title,
        byline.as_deref(),
        lang.as_deref(),
        site_name.as_deref(),
        published_time
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(archive)
}

async fn update_archive_to_error(
    tx: &mut AppTx,
    archive_id: Uuid,
    error: &archive::Error,
) -> ResponseResult<Archive> {
    let error = serde_json::to_value(error).context("Failed to serialize error")?;

    let archive = sqlx::query_as!(
        Archive,
        r#"
            update archives
            set status = $2,
                error = $3,
                extracted_html = null,
                extracted_title = null,
                byline = null,
                lang = null,
                site_name = null,
                published_time = null
            where id = $1
            returning id, bookmark_id, created_at, status as "status: _", error as "error: Json<archive::Error>", extracted_title, extracted_html, byline, lang, site_name, published_time as "published_time: OffsetDateTime"
        "#,
        archive_id,
        Status::Error as Status,
        error,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(archive)
}

pub async fn by_bookmark_id(tx: &mut AppTx, bookmark_id: Uuid) -> ResponseResult<Option<Archive>> {
    let archive = sqlx::query_as!(
        Archive,
        r#"
        select id, bookmark_id, created_at, status as "status: _", error as "error: Json<archive::Error>", extracted_title, extracted_html, byline, lang, site_name, published_time as "published_time: OffsetDateTime"
        from archives
        where bookmark_id = $1
        "#,
        bookmark_id,
    )
    .fetch_optional(&mut **tx)
    .await?;

    Ok(archive)
}

pub async fn by_id(tx: &mut AppTx, archive_id: Uuid) -> ResponseResult<Archive> {
    let archive = sqlx::query_as!(
        Archive,
        r#"
        select id, bookmark_id, created_at, status as "status: _", error as "error: Json<archive::Error>", extracted_title, extracted_html, byline, lang, site_name, published_time as "published_time: OffsetDateTime"
        from archives
        where id = $1
        "#,
        archive_id,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(archive)
}

pub async fn find_one_pending(tx: &mut AppTx) -> ResponseResult<Option<Uuid>> {
    let archive = sqlx::query!(
        r#"
        select id
        from archives
        where status = $1
        "#,
        Status::Pending as Status,
    )
    .fetch_optional(&mut **tx)
    .await?
    .map(|r| r.id);

    Ok(archive)
}

pub async fn delete_by_bookmark_id(tx: &mut AppTx, bookmark_id: Uuid) -> ResponseResult<()> {
    sqlx::query!("delete from archives where bookmark_id = $1", bookmark_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}
