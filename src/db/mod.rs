use anyhow::{Context, Result};
use sqlx::PgPool;

pub mod all;
pub mod ap_users;
pub mod follows;
pub mod run_migrations;
pub use ap_users::ApUser;
pub mod items;
pub mod layout;
pub mod links;
pub use links::{LinkDestination, LinkDestinationWithChildren, LinkWithContent};
pub mod lists;
pub use lists::{List, ListWithLinks, ListWithMetadata};
pub mod users;
use url::Url;
pub use users::User;
pub mod bookmarks;
pub mod migration_hooks;
pub use bookmarks::Bookmark;
pub mod search;

pub async fn migrate(pool: &PgPool, base_url: &Url, up_to_version: Option<i64>) -> Result<()> {
    tracing::info!("Migrating the database...");
    let migrator = sqlx::migrate!("./migrations");
    let mut conn = pool.acquire().await?;
    run_migrations::run_migrations(&migrator, &mut conn, base_url, up_to_version).await?;
    tracing::info!("Database migrated.");

    Ok(())
}

pub async fn pool(url: &str) -> Result<sqlx::PgPool> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await
        .context("Failed to create database connection pool")
}

pub type AppTx = sqlx::Transaction<'static, sqlx::Postgres>;
