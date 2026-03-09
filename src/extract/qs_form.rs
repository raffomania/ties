use anyhow::Context;
use axum::{
    RequestExt,
    extract::{FromRequest, RawForm, Request},
};

use crate::response_error::ResponseError;

pub struct QsForm<T>(pub T);

impl<T, S> FromRequest<S> for QsForm<T>
where
    T: serde::de::DeserializeOwned,
    S: std::marker::Sync,
{
    type Rejection = ResponseError;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let RawForm(bytes) = req.extract().await.context("Failed to extract form")?;
        Ok(Self(
            serde_qs::Config::new()
                .use_form_encoding(true)
                .deserialize_bytes(&bytes)
                .context("Failed to parse form")?,
        ))
    }
}
