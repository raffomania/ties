use std::{path::PathBuf, time::Duration};

use activitypub_federation::config::{FederationConfig, FederationMiddleware};
use anyhow::{Context, anyhow};
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use listenfd::ListenFd;
use sqlx::PgPool;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tower_sessions::ExpiredDeletion;
use url::Url;

use crate::{
    cli::ListenArgs,
    db::{self},
    federation, oidc, routes,
};

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub base_url: Url,
    pub demo_mode: bool,
    pub oidc_state: oidc::State,
    pub federation_config: FederationConfig<federation::Context>,
}

pub async fn app(state: AppState) -> anyhow::Result<Router> {
    let session_store = tower_sessions_sqlx_store::PostgresStore::new(state.pool.clone());
    session_store.migrate().await?;
    tokio::task::spawn(
        session_store
            .clone()
            .continuously_delete_expired(tokio::time::Duration::from_secs(6 * 60 * 60)),
    );

    if state.demo_mode {
        tokio::task::spawn(periodically_wipe_all_data(state.pool.clone()));
    }

    let cookie_inactivity_limit = if state.demo_mode {
        tower_sessions::cookie::time::Duration::hours(1)
    } else {
        tower_sessions::cookie::time::Duration::weeks(2)
    };

    let session_service = tower_sessions::SessionManagerLayer::new(session_store)
        .with_secure(true)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(tower_sessions::Expiry::OnInactivity(
            cookie_inactivity_limit,
        ));

    Ok(Router::new()
        .merge(routes::users::router())
        .merge(routes::index::router())
        .merge(routes::lists::router())
        .merge(routes::bookmarks::router())
        .merge(routes::links::router())
        .merge(routes::federation::router())
        .merge(routes::assets::router().with_state(()))
        // TODO add layer to use the same URL for AP and HTML
        // this should simplify things and be more error tolerant for other services
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(session_service)
                .layer(FederationMiddleware::new(state.federation_config.clone())),
        )
        .with_state(state))
}

pub async fn start(
    listen: ListenArgs,
    base_url: Url,
    app: Router,
    tls_cert: Option<PathBuf>,
    tls_key: Option<PathBuf>,
) -> anyhow::Result<()> {
    let handle = axum_server::Handle::new();

    let listener = if listen.listenfd {
        // Graceful shutdown is somehow broken with listenfd at the moment
        tokio::spawn(shutdown_signal(handle.clone(), false));
        let mut listenfd = ListenFd::from_env();
        let listener = listenfd
            .take_tcp_listener(0)?
            .ok_or(anyhow!("No systemfd TCP socket found"))?;
        listener.set_nonblocking(true)?;
        tokio::net::TcpListener::from_std(listener)?
    } else if let Some(listen_address) = listen.listen {
        tokio::spawn(shutdown_signal(handle.clone(), true));
        tokio::net::TcpListener::bind(format!("{listen_address}")).await?
    } else {
        anyhow::bail!(
            "Please specify either an address and port to listen on, or use the --listenfd flag."
        );
    };

    let listening_on = listener.local_addr()?;

    if let (Some(cert), Some(key)) = (tls_cert, tls_key) {
        tracing::info!("Using TLS files at: {cert:?}, {key:?}");
        tracing::info!("Listening on {listening_on}");
        tracing::info!("Access the server at {base_url}");

        let config = RustlsConfig::from_pem_file(cert, key).await?;
        axum_server::from_tcp_rustls(listener.into_std()?, config)?
            .handle(handle)
            .serve(app.into_make_service())
            .await?;
    } else {
        tracing::info!("No TLS certificate specified, not using TLS");
        tracing::info!("Listening on {listening_on}");
        tracing::info!("Access the server at {base_url}");

        axum_server::from_tcp(listener.into_std()?)?
            .handle(handle)
            .serve(app.into_make_service())
            .await?;
    }

    Ok(())
}

async fn shutdown_signal<A>(handle: axum_server::Handle<A>, graceful: bool)
where
    A: axum_server::Address,
{
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .context("failed to install Ctrl+C handler")
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .context("failed to install signal handler")?
            .recv()
            .await;

        Ok::<(), anyhow::Error>(())
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!(
        "Received termination signal - waiting 10 seconds to close existing connections"
    );
    if graceful {
        handle.graceful_shutdown(Some(Duration::from_secs(10)));
    } else {
        handle.shutdown();
    }
}

async fn periodically_wipe_all_data(pool: PgPool) -> anyhow::Result<()> {
    // interval: every hour
    let period = tokio::time::Duration::from_secs(10);
    let mut interval = tokio::time::interval(period);
    // First interval completes immediately, but we want to wait
    // before doing the first deletion to give users time
    // to react to the warning
    interval.tick().await;
    tracing::warn!("Demo mode enabled - will periodically wipe ALL DATA every {period:?}.");

    loop {
        interval.tick().await;
        let res = wipe_all_data(&pool).await;
        if let Err(e) = res {
            tracing::error!("{e:?}");
        }
    }
}

async fn wipe_all_data(pool: &PgPool) -> anyhow::Result<()> {
    tracing::info!("Wiping all data!");
    let mut tx = pool.begin().await?;
    db::all::wipe_all_data(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}
