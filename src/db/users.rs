use sqlx::{FromRow, query_as};
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
pub struct User {
    pub id: Uuid,

    // TODO this is only used in tests so far, which breaks
    // `#[expect(dead_code)]` for some reason
    #[allow(dead_code)]
    pub username: String,

    // Password-based login data
    pub password_hash: Option<String>,

    // SSO-related data
    #[expect(dead_code)]
    pub email: Option<String>,
    #[expect(dead_code)]
    pub oidc_id: Option<String>,

    // ActivityPub data
    #[allow(dead_code)]
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

// TODO this needs to create an AP user?
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
        (email, oidc_id, username, ap_user_id)
        values ($1, $2, $3, $4)
        returning *"#,
        create_user.email,
        create_user.oidc_id,
        create_user.username,
        ap_user.id
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(user)
}

pub async fn insert(
    tx: &mut AppTx,
    create_user: CreateUser,
    base_url: &Url,
) -> ResponseResult<User> {
    let hashed_password = hash_password(&create_user.password)?;
    let create_ap_user = CreateApUser::new_local(base_url, create_user.username.clone())?;
    let ap_user = super::ap_users::insert(tx, create_ap_user).await?;

    let user = query_as!(
        User,
        r#"
        insert into users
        (username, password_hash, ap_user_id)
        values ($1, $2, $3)
        returning *
        "#,
        create_user.username,
        hashed_password,
        ap_user.id
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

pub async fn by_ap_user_id(tx: &mut AppTx, ap_user_id: Uuid) -> ResponseResult<Option<User>> {
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

pub async fn create_if_not_exists(
    tx: &mut AppTx,
    create: CreateUser,
    base_url: &Url,
) -> ResponseResult<User> {
    let username = create.username.clone();
    let user = by_username(tx, &username).await;
    let actual_user = match user {
        Err(ResponseError::NotFound) => {
            tracing::info!("Creating admin user '{username}'");
            insert(tx, create, base_url).await?
        }
        Ok(actual_user) => {
            tracing::debug!("Admin user '{username}' exists, not creating");
            actual_user
        }
        Err(other) => return Err(other),
    };
    Ok(actual_user)
}
