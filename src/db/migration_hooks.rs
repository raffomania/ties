//! Rust-based hooks for running alongside SQL migrations.
//! These are interleaved with SQL-based migrations from the top-level
//! `migrations` folder.
//!
//! When writing these hooks, only use dynamic sqlx queries - since
//! the schema might change in the future, but queries in here should never
//! change.
//!
//! Note that these don't behave like SQL migrations: they are not checksummed,
//! we never record whether a hook has run, and don't check for previously run
//! hooks that are now missing.

use anyhow::Result;
use sqlx::PgTransaction;
use url::Url;

mod generate_bookmark_ap_ids;
mod generate_missing_ap_users;

#[allow(
    clippy::inconsistent_digit_grouping,
    reason = "Used to make the numbers readable"
)]
pub async fn run_before(
    previous_migration: &sqlx::migrate::Migration,
    tx: &mut PgTransaction<'_>,
    base_url: &Url,
) -> Result<()> {
    // The simplest way to dispatch hooks for specific migrations.
    // Feel free to refactor this if it becomes unwieldly in the future.
    match previous_migration.version {
        2025_10_14_160454 => {
            generate_missing_ap_users::migrate(tx, base_url).await?;
        }
        2025_11_05_102754 => {
            generate_bookmark_ap_ids::migrate(tx, base_url).await?;
        }
        _ => {}
    }

    Ok(())
}
