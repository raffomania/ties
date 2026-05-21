mod archive;
mod authentication;
pub mod cli;
mod db;
mod extract;
mod form_errors;
mod forms;
mod oidc;
mod response_error;
mod routes;
pub mod server;
mod views;

mod built_version;
mod date_time;
mod federation;
mod htmf_response;
#[cfg(debug_assertions)]
mod insert_demo_data;
#[cfg(test)]
mod tests;
#[cfg(debug_assertions)]
mod tracing_format;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
