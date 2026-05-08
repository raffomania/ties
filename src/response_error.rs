use axum::{
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use thiserror::Error;

pub type ResponseResult<T> = core::result::Result<T, ResponseError>;

#[derive(Debug, Error)]
pub enum ResponseError {
    #[error("Not Found")]
    NotFound,
    #[error("Authentication Failed")]
    NotAuthenticated,
    #[error("Internal Error")]
    Anyhow(#[from] anyhow::Error),
    #[error("Internal Error")]
    UrlParseError(#[from] url::ParseError),
    #[error("Internal Error")]
    FederationError(#[from] activitypub_federation::error::Error),
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> Response {
        tracing::error!("{self:?}");
        match self {
            ResponseError::NotFound => (StatusCode::NOT_FOUND, self.to_string()).into_response(),
            ResponseError::NotAuthenticated => Redirect::to("/login").into_response(),
            ResponseError::Anyhow(_)
            | ResponseError::UrlParseError(_)
            | ResponseError::FederationError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
            }
        }
    }
}

/// Map [`ResponseError::NotFound`] to `None`
pub fn into_option<T>(result: ResponseResult<T>) -> ResponseResult<Option<T>> {
    match result {
        Ok(val) => Ok(Some(val)),
        Err(ResponseError::NotFound) => Ok(None),
        Err(err) => Err(err),
    }
}

impl From<sqlx::Error> for ResponseError {
    fn from(value: sqlx::Error) -> Self {
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "It's explicitly wanted to catch all future variants as well."
        )]
        match value {
            sqlx::Error::RowNotFound => Self::NotFound,
            other => Self::Anyhow(other.into()),
        }
    }
}
