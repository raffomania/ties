use std::hash::{DefaultHasher, Hash, Hasher};

use sqlx::{FromRow, query_as};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use super::AppTx;
use crate::{
    authentication::hash_password,
    forms::{
        ap_users::CreateApUser,
        users::{CreateOidcUser, CreateUser},
    },
    response_error::{ResponseError, ResponseResult},
};

#[derive(FromRow, Debug)]
#[allow(dead_code, reason = "Kept as reference on the DB schema.")]
pub struct User {
    pub id: Uuid,

    pub username: String,
    pub invited_by: Option<Uuid>,

    // Password-based login data
    pub password_hash: Option<String>,

    // SSO-related data
    pub email: Option<String>,
    pub oidc_id: Option<String>,

    // ActivityPub data
    pub ap_user_id: Uuid,
}

pub async fn by_oidc_id(tx: &mut AppTx, oidc_id: &str) -> ResponseResult<User> {
    let user = query_as!(
        User,
        r#"
        select * from users
        where oidc_id = $1
        "#,
        oidc_id
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(user)
}

pub async fn by_id(tx: &mut AppTx, id: Uuid) -> ResponseResult<User> {
    let user = query_as!(
        User,
        r#"
        select * from users
        where id = $1
        "#,
        id
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(user)
}

pub async fn insert_oidc(
    tx: &mut AppTx,
    create_user: CreateOidcUser,
    base_url: &Url,
) -> ResponseResult<User> {
    let create_ap_user = CreateApUser::new_local(base_url, create_user.username.clone())?;
    let ap_user = super::ap_users::insert(tx, create_ap_user).await?;

    let user = query_as!(
        User,
        r#"
        insert into users
        (email, oidc_id, username, ap_user_id, invited_by)
        values ($1, $2, $3, $4, null)
        returning *"#,
        create_user.email,
        create_user.oidc_id,
        create_user.username,
        ap_user.id,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(user)
}

pub async fn insert(
    tx: &mut AppTx,
    create_user: CreateUser,
    invited_by_user_id: Option<Uuid>,
    base_url: &Url,
) -> ResponseResult<User> {
    let hashed_password = hash_password(&create_user.password)?;
    let create_ap_user = CreateApUser::new_local(base_url, create_user.username.clone())?;
    let ap_user = super::ap_users::insert(tx, create_ap_user).await?;

    let user = query_as!(
        User,
        r#"
        insert into users
        (username, password_hash, ap_user_id, invited_by)
        values ($1, $2, $3, $4)
        returning *
        "#,
        create_user.username,
        hashed_password,
        ap_user.id,
        invited_by_user_id
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(user)
}

pub async fn by_username(tx: &mut AppTx, username: &str) -> ResponseResult<User> {
    let user = query_as!(
        User,
        r#"
        select * from users
        where username = $1
        "#,
        username
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(user)
}

pub async fn local_by_ap_user_id(tx: &mut AppTx, ap_user_id: Uuid) -> ResponseResult<Option<User>> {
    let user = query_as!(
        User,
        r#"
        select * from users
        where ap_user_id = $1
        "#,
        ap_user_id
    )
    .fetch_optional(&mut **tx)
    .await?;

    Ok(user)
}

pub async fn create_root_admin_if_not_exists(
    tx: &mut AppTx,
    create: CreateUser,
    base_url: &Url,
) -> ResponseResult<User> {
    let username = create.username.clone();
    let user = by_username(tx, &username).await;
    let actual_user = match user {
        Err(ResponseError::NotFound) => {
            tracing::info!("Creating user '{username}'");
            insert(tx, create, None, base_url).await?
        }
        Ok(actual_user) => {
            tracing::debug!("Admin user '{username}' exists, not creating");
            actual_user
        }
        Err(other) => return Err(other),
    };
    Ok(actual_user)
}

/// Cheap-to-compute validation signal for a cached feed.
#[derive(FromRow, Clone, PartialEq, Hash)]
pub struct FeedCacheSignal {
    pub username: String,
    pub newest_bookmark_created_at: Option<OffsetDateTime>,
    pub newest_list_created_at: Option<OffsetDateTime>,
    pub newest_link_created_at: Option<OffsetDateTime>,
    pub public_list_count: i64,
    pub link_count: i64,
}

impl FeedCacheSignal {
    /// Little wrapper around [Hash::hash]
    pub fn get_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

pub async fn feed_cache_signal(
    tx: &mut AppTx,
    ap_user_id: Uuid,
) -> ResponseResult<Option<FeedCacheSignal>> {
    let signal = query_as!(
        FeedCacheSignal,
        r#"
        select ap_users.username,
            (select max(created_at) from bookmarks
                where bookmarks.ap_user_id = ap_users.id)
                as newest_bookmark_created_at,
            (select max(created_at) from lists
                where lists.ap_user_id = ap_users.id and not lists.private)
                as newest_list_created_at,
            (select max(created_at) from links
                where links.user_id = users.id)
                as newest_link_created_at,
            (select count(*) from lists
                where lists.ap_user_id = ap_users.id and not lists.private)
                as "public_list_count!",
            (select count(*) from links
                where links.user_id = users.id)
                as "link_count!"
        from ap_users
        join users on users.ap_user_id = ap_users.id
        where ap_users.id = $1
        "#,
        ap_user_id
    )
    .fetch_optional(&mut **tx)
    .await?;

    Ok(signal)
}
