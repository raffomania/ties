use anyhow::Result;
use axum::{extract::FromRequestParts, http::request::Parts};

use crate::{db::AppTx, response_error::ResponseError, server::AppState};

pub struct Tx(pub AppTx);

impl FromRequestParts<AppState> for Tx {
    type Rejection = ResponseError;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let tx = state.pool.begin().await?;

        Ok(Self(tx))
    }
}
