use sqlx::query;

use super::AppTx;
use crate::response_error::ResponseResult;

pub async fn wipe_all_data(tx: &mut AppTx) -> ResponseResult<()> {
    let tables = query!(
        r#"
        select schemaname as "schemaname!", tablename as "tablename!"
        from pg_tables
        where schemaname in ('public', 'tower_sessions')
        and tablename != '_sqlx_migrations';"#
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|record| format!("{}.{}", record.schemaname, record.tablename))
    .collect::<Vec<_>>()
    .join(",");

    query(&format!("truncate {tables}"))
        .execute(&mut **tx)
        .await?;

    Ok(())
}
