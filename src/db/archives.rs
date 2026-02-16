use time::OffsetDateTime;
use uuid::Uuid;

use crate::{db::AppTx, response_error::ResponseResult};

#[derive(sqlx::Type, Debug)]
#[sqlx(type_name = "archive_status")]
pub enum Status {
    Success,
    Error,
}

#[derive(sqlx::FromRow, Debug)]
pub struct Archive {
    pub id: Uuid,

    pub bookmark_id: Uuid,

    pub created_at: OffsetDateTime,
    pub status: Status,
    pub error_description: Option<String>,
    pub extracted_html: Option<String>,
}

pub async fn insert(
    tx: &mut AppTx,
    bookmark_id: Uuid,
    article: &anyhow::Result<legible::Article>,
) -> ResponseResult<Archive> {
    let id = Uuid::new_v4();
    let (status, error_description, extracted_html) = match article {
        Ok(article) => (Status::Success, None, Some(&article.content)),
        Err(e) => (Status::Error, Some(e.to_string()), None),
    };
    let archive = sqlx::query_as!(
        Archive,
        r#"
        insert into archives
        (id, bookmark_id, status, error_description, extracted_html)
        values ($1, $2, $3, $4, $5)
        returning id, bookmark_id, created_at, status as "status: _", error_description, extracted_html
        "#,
        id,
        bookmark_id,
        status as Status,
        error_description,
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
        select id, bookmark_id, created_at, status as "status: _", error_description, extracted_html
        from archives
        where bookmark_id = $1
        "#,
        bookmark_id,
    )
    .fetch_optional(&mut **tx)
    .await?;

    Ok(archive)
}
