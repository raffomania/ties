use std::sync::atomic::AtomicUsize;

use axum::{Router, http::StatusCode};
use sqlx::{Pool, Postgres};
use tokio::net::TcpListener;
use url::Url;

use super::request_builder::RequestBuilder;
use crate::{
    db::{self, AppTx},
    federation,
    forms::users::CreateUser,
    server::{AppState, app},
};

const TEST_USER_USERNAME: &str = "testuser";
const TEST_USER_PASSWORD: &str = "testpassword";

static NEXT_TEST_APP_PORT: AtomicUsize = AtomicUsize::new(4041);

pub struct TestApp {
    pub logged_in_cookie: Option<String>,
    pub router: Router,
    pub pool: Pool<Postgres>,
    pub base_url: Url,
    pub state: AppState,
    pub port: usize,
}

impl TestApp {
    pub async fn new() -> Self {
        let port = NEXT_TEST_APP_PORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let base_url = Url::parse(&format!("http://localhost:{port}",))
            .expect("Failed to parse URL for test instance");
        let pool = super::db::new_test_pool().await;

        crate::db::migrate(&pool, &base_url, None)
            .await
            .expect("Failed to migrate database");

        let state = AppState {
            pool: pool.clone(),
            base_url: base_url.clone(),
            demo_mode: false,
            oidc_state: crate::oidc::State::NotConfigured,
            federation_config: federation::config::new_config(pool.clone(), base_url.clone())
                .await
                .unwrap(),
        };

        TestApp {
            router: app(state.clone()).await.unwrap(),
            pool,
            logged_in_cookie: None,
            base_url,
            state,
            port,
        }
    }

    pub async fn tx(&self) -> AppTx {
        self.pool
            .begin()
            .await
            .expect("Failed to start new transaction")
    }

    pub fn req(&mut self) -> RequestBuilder {
        let mut req = RequestBuilder::new(&self.router);
        if let Some(cookie) = &self.logged_in_cookie {
            req = req.header(axum::http::header::COOKIE, cookie);
        }
        req
    }

    /// Since there's no route for creating users yet, we're doing this via the
    /// DB for now.
    pub async fn create_user(&self, username: &str, password: &str) -> db::User {
        let mut tx = self
            .pool
            .begin()
            .await
            .expect("Failed to create transaction");
        let user = crate::db::users::create_if_not_exists(
            &mut tx,
            CreateUser {
                username: username.to_string(),
                password: password.to_string(),
            },
            &self.base_url,
        )
        .await
        .expect("Failed to create new user");
        tx.commit().await.expect("Failed to commit transaction");

        user
    }

    pub async fn create_test_user(&self) -> db::User {
        self.create_user(TEST_USER_USERNAME, TEST_USER_PASSWORD)
            .await
    }

    pub async fn login_test_user(&mut self) {
        self.login_user(TEST_USER_USERNAME, TEST_USER_PASSWORD)
            .await;
    }

    pub async fn login_user(&mut self, username: &str, password: &str) {
        self.logged_in_cookie = None;
        let login_page = self.req().get("/login").await.test_page().await;

        let input = crate::forms::users::Login {
            credentials: crate::forms::users::Credentials {
                username: username.to_string(),
                password: password.to_string(),
            },
            previous_uri: None,
        };

        let login_response = login_page
            .expect_status(StatusCode::SEE_OTHER)
            .fill_form("form", &input)
            .await;

        let cookie = login_response.headers().get("Set-Cookie").unwrap();
        let cookie = cookie.to_str().unwrap().split_once(';').unwrap().0;
        assert!(!cookie.is_empty());

        self.logged_in_cookie = Some(cookie.to_string());
    }

    pub async fn serve(&self) {
        let listener = TcpListener::bind(format!("localhost:{}", self.port))
            .await
            .unwrap();
        let router = self.router.clone();

        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
    }
}
