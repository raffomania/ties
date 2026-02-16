use anyhow::Context;
use sqlx::types::Json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{archive, db::AppTx, response_error::ResponseResult};

#[derive(sqlx::Type, Debug)]
#[sqlx(type_name = "archive_status")]
pub enum Status {
    Success,
    Error,
    Pending,
}

#[derive(sqlx::FromRow, Debug)]
pub struct Archive {
    pub id: Uuid,

    pub bookmark_id: Uuid,

    pub created_at: OffsetDateTime,
    pub status: Status,
    pub error: Option<Json<archive::Error>>,
    pub extracted_html: Option<String>,
}

pub async fn insert(
    tx: &mut AppTx,
    bookmark_id: Uuid,
    article: &Result<legible::Article, archive::Error>,
) -> ResponseResult<Archive> {
    let id = Uuid::new_v4();
    let (status, error, extracted_html) = match article {
        Ok(article) => (Status::Success, None, Some(&article.content)),
        Err(e) => (
            Status::Error,
            Some(serde_json::to_value(e).context("Failed to serialize error")?),
            None,
        ),
    };
    let archive = sqlx::query_as!(
        Archive,
        r#"
        insert into archives
        (id, bookmark_id, status, error, extracted_html)
        values ($1, $2, $3, $4, $5)
        returning id, bookmark_id, created_at, status as "status: _", error as "error: Json<archive::Error>", extracted_html
        "#,
        id,
        bookmark_id,
        status as Status,
        error,
        extracted_html
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(archive)
}

pub async fn by_bookmark_id(tx: &mut AppTx, bookmark_id: Uuid) -> ResponseResult<Option<Archive>> {
    let archive = sqlx::query_as!(
        Archive,
        r#"
        select id, bookmark_id, created_at, status as "status: _", error as "error: Json<archive::Error>", extracted_html
        from archives
        where bookmark_id = $1
        "#,
        bookmark_id,
    )
    .fetch_optional(&mut **tx)
    .await?;

    Ok(archive)
}
