use anyhow::Result;
use sqlx::{FromRow, PgTransaction};
use url::Url;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct Bookmark {
    id: Uuid,
}

pub async fn migrate(tx: &mut PgTransaction<'_>, base_url: &Url) -> Result<()> {
    let bookmarks_without_ap_id = sqlx::query("select id from bookmarks where ap_id is null")
        .fetch_all(&mut **tx)
        .await?;

    for row in bookmarks_without_ap_id {
        let bookmark = Bookmark::from_row(&row)?;
        let ap_id = base_url
            .join("/ap/bookmark/")?
            .join(&bookmark.id.to_string())?;
        sqlx::query(
            "
            update bookmarks
            set ap_id = $1
            where id = $2
        ",
        )
        .bind(ap_id.to_string())
        .bind(bookmark.id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}
