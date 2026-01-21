use std::{net::SocketAddr, path::PathBuf};

use anyhow::{Result, anyhow};
use clap::{Args, Parser, Subcommand};
use garde::Validate;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

#[cfg(debug_assertions)]
use crate::insert_demo_data::insert_demo_data;
use crate::{
    db, federation,
    forms::users::CreateUser,
    oidc,
    server::{self, AppState},
};

#[derive(Parser, Debug)]
#[clap(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    #[clap(flatten)]
    config: SharedConfig,
}

#[derive(Args, Debug)]
struct SharedConfig {
    #[clap(env, long, hide_env_values = true)]
    database_url: String,
    /// Public URL the server is reachable at.
    /// This forms the domain part of user handles, e.g. "rafael@ties.pub", and
    /// cannot be changed once the first user has been created.
    #[clap(long, env)]
    base_url: Url,
}

#[derive(Args, Debug)]
#[group(requires_all = ["oidc_client_id", "oidc_client_secret", "oidc_issuer_url", "oidc_issuer_name"])]
pub struct OidcArgs {
    /// To use OIDC, all options beginning with `oidc` must be set.
    /// We support RS*, PS*, or HS* signature algorithms.
    /// Configure your redirect URL to be `{base_url}/login_oidc_redirect`.
    #[clap(long, env, required = false)]
    pub oidc_client_id: String,
    // TODO use redact::Secret for this
    #[clap(hide_env_values = true, long, env, required = false)]
    pub oidc_client_secret: redact::Secret<String>,
    #[clap(long, env, required = false)]
    pub oidc_issuer_url: String,
    /// This will be displayed on the login page.
    #[clap(long, env, required = false)]
    pub oidc_issuer_name: String,
}

#[derive(Parser, Debug)]
enum Command {
    /// Migrate the database, then start the server
    Start {
        #[clap(flatten)]
        listen: ListenArgs,
        /// TLS cert location.
        /// If set, requires `tls-key` to be set as well.
        /// If both `tls-key` and `tls-cert` are unset, no TLS is used.
        #[clap(long, env, requires = "tls_key")]
        tls_cert: Option<PathBuf>,
        /// TLS key location.
        /// If set, requires `tls-cert` to be set as well.
        /// If both `tls-key` and `tls-cert` are unset, no TLS is used.
        #[clap(long, env, requires = "tls_cert")]
        tls_key: Option<PathBuf>,
        #[clap(flatten)]
        admin_credentials: AdminCredentials,
        /// Replace the login page with an anonymous one-click signup and delete
        /// all data periodically.
        #[clap(long, env, default_value = "false")]
        demo_mode: bool,
        #[clap(flatten)]
        oidc_args: Option<OidcArgs>,
    },
    Db {
        #[clap(subcommand)]
        command: DbCommand,
    },
    #[cfg(debug_assertions)]
    /// Put some demo data into the database
    InsertDemoData {
        #[clap(flatten)]
        dev_user_credentials: AdminCredentials,
    },
}

#[derive(Args, Debug)]
#[group(multiple = true, requires_all = ["username", "password"])]
struct AdminCredentials {
    #[clap(env = "ADMIN_USERNAME", long = "admin_username")]
    /// Create an admin user if it does not exist yet.
    username: Option<String>,
    #[clap(
        env = "ADMIN_PASSWORD",
        long = "admin_password",
        hide_env_values = true
    )]
    /// Password for the admin user.
    password: Option<redact::Secret<String>>,
}

impl From<AdminCredentials> for Option<CreateUser> {
    fn from(value: AdminCredentials) -> Self {
        if let (Some(username), Some(password)) = (value.username, value.password) {
            Some(CreateUser {
                username,
                password: password.expose_secret().clone(),
            })
        } else {
            None
        }
    }
}

#[derive(Subcommand, Debug)]
enum DbCommand {
    Migrate,
}

#[derive(Args, Debug)]
#[group(required = true, multiple = true)]
pub struct ListenArgs {
    /// Format: `ip:port`.
    #[clap(long, env, value_name = "SOCKET_ADDRESS")]
    pub listen: Option<SocketAddr>,
    /// Take a socket using the systemd socket passing protocol and listen on
    /// it. If set, will override the `listen` argument.
    #[clap(long, env)]
    pub listenfd: bool,
}

pub async fn run() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow!("Failed to set default crypto provider"))?;

    let cli = Cli::parse();

    tracing::debug!("{cli:#?}");

    let base_url = cli.config.base_url;
    match cli.command {
        Command::Start {
            listen: listen_address,
            admin_credentials,
            tls_cert,
            tls_key,
            demo_mode,
            oidc_args,
        } => {
            let pool = db::pool(&cli.config.database_url).await?;

            db::migrate(&pool, &base_url, None).await?;

            if let Some(create) = Option::<CreateUser>::from(admin_credentials) {
                if let Err(e) = create.validate() {
                    return Err(anyhow!("Invalid credentials for admin user provided:\n{e}"));
                }
                let mut tx = pool.begin().await?;
                db::users::create_if_not_exists(&mut tx, create, &base_url).await?;
                tx.commit().await?;
            }

            let oidc_state = oidc::State::initialize(&base_url, oidc_args).await;

            let app = server::app(AppState {
                pool: pool.clone(),
                base_url: base_url.clone(),
                demo_mode,
                oidc_state,
                federation_config: federation::config::new_config(pool, base_url.clone()).await?,
            })
            .await?;
            server::start(listen_address, base_url.clone(), app, tls_cert, tls_key).await?;
        }
        Command::Db {
            command: DbCommand::Migrate,
        } => {
            let pool = db::pool(&cli.config.database_url).await?;
            db::migrate(&pool, &base_url, None).await?;
        }
        #[cfg(debug_assertions)]
        Command::InsertDemoData {
            dev_user_credentials,
        } => {
            let pool = db::pool(&cli.config.database_url).await?;
            insert_demo_data(
                &pool,
                Option::<CreateUser>::from(dev_user_credentials),
                &base_url,
            )
            .await?;
        }
    }

    Ok(())
}
