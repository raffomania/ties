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
mod session;
mod views;

mod atom;

mod built_version;
mod date_time;
mod federation;
mod htmf_response;
pub mod http_cache;
#[cfg(debug_assertions)]
mod insert_demo_data;
pub mod reverse_routes;
#[cfg(test)]
mod tests;
#[cfg(debug_assertions)]
mod tracing_filter;
#[cfg(debug_assertions)]
mod tracing_format;

use anyhow::Result;
// Shut up the cargo unused dependencies lint for this specific dependency.
// It's only used in development and there's no good alternative to fix this.
#[cfg(not(debug_assertions))]
#[expect(unused_imports)]
use fake;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
