#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![warn(clippy::pedantic)]
#![expect(clippy::missing_errors_doc)]
#![expect(clippy::redundant_closure_for_method_calls)]

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

mod date_time;
mod federation;
mod htmf_response;
#[cfg(debug_assertions)]
mod insert_demo_data;
#[cfg(test)]
mod tests;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
