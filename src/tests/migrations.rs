#![expect(clippy::inconsistent_digit_grouping)]

use anyhow::Result;
use sqlx::Row;
use url::Url;
use uuid::Uuid;

use crate::{db, forms::users::CreateUser};

#[test_log::test(tokio::test)]
async fn test_generate_missing_ap_users_migration() -> Result<()> {
    // Create a test database pool without running migrations
    let pool = super::util::db::new_test_pool().await;
    let base_url = Url::parse("http://localhost:3000")?;

    // First, run up to and including the migration that creates the ap_users table
    // and adds the ap_user_id column, but don't run the Rust
    // migration that generates AP users yet
    let sql_migration_version = Some(2025_01_27_102308);
    db::migrate(&pool, &base_url, sql_migration_version).await?;

    // Create a user without an associated ap_user_id
    // Don't use static queries here since they don't reflect the newest schema
    let mut conn = pool.acquire().await?;
    let user_id: Uuid = sqlx::query(
        r"
        insert into users (username)
        values ($1)
        returning id
        ",
    )
    .bind("testuser")
    .fetch_one(&mut *conn)
    .await?
    .get(0);

    // Verify the user was created without an ap_user_id
    let ap_user_id: Option<Uuid> = sqlx::query("select ap_user_id from users where id = $1")
        .bind(user_id)
        .fetch_one(&mut *conn)
        .await?
        .get(0);

    assert!(
        ap_user_id.is_none(),
        "User should not have an ap_user_id before migration"
    );

    // Now run the Rust migration that generates AP users for existing users
    db::migrate(&pool, &base_url, None).await?;

    // Verify that the user now has an associated ap_user_id
    let ap_user_id: Uuid = sqlx::query!("select ap_user_id from users where id = $1", user_id)
        .fetch_one(&mut *conn)
        .await?
        .ap_user_id;

    // Verify that the AP user was created with correct data
    let row = sqlx::query!(
        "select ap_id, username, id, inbox_url from ap_users where id = $1",
        ap_user_id
    )
    .fetch_one(&mut *conn)
    .await?;

    assert_eq!(
        row.username, "testuser",
        "AP user should have the same username"
    );
    assert_eq!(
        row.ap_id,
        base_url
            .join("/ap/user/")?
            .join(&row.id.to_string())?
            .to_string(),
        "AP ID should be correctly formatted"
    );
    assert_eq!(
        row.inbox_url,
        base_url
            .join("/ap/inbox/")?
            .join(&row.id.to_string())?
            .to_string(),
        "Inbox URL should be correctly formatted"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_generate_missing_bookmark_ap_ids_migration() -> Result<()> {
    // Create a test database pool without running migrations
    let pool = super::util::db::new_test_pool().await;
    let base_url = Url::parse("http://localhost:3000")?;

    // First, run up to and including the migration that creates the ap_users table
    // and adds the ap_user_id column, but don't run the Rust
    // migration that generates AP users yet
    let sql_migration_version = Some(2025_11_05_100411);
    db::migrate(&pool, &base_url, sql_migration_version).await?;

    // Create bookmark without an ap id
    let mut tx = pool.begin().await?;

    let create_user = CreateUser {
        username: "testuser".to_string(),
        password: "testpassword".to_string(),
    };

    let user = db::users::insert(&mut tx, create_user, &base_url).await?;
    let id = Uuid::new_v4();
    let title = "test title";
    let url = "https://ties.rafa.ee";

    let bookmark_id: Uuid = sqlx::query(
        r"
        insert into bookmarks
        (id, user_id, url, title)
        values ($1, $2, $3, $4)
        returning id
        ",
    )
    .bind(id)
    .bind(user.id)
    .bind(url)
    .bind(title)
    .fetch_one(&mut *tx)
    .await?
    .get(0);

    tx.commit().await?;

    // Run up to the newest migration
    db::migrate(&pool, &base_url, None).await?;

    let mut tx = pool.begin().await?;

    let bookmark = sqlx::query!(r"select ap_id from bookmarks where id = $1", id)
        .fetch_one(&mut *tx)
        .await?;

    assert_eq!(
        bookmark.ap_id,
        format!("{base_url}ap/bookmark/{bookmark_id}")
    );

    tx.commit().await?;

    Ok(())
}
