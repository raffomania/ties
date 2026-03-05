use serde::{Deserialize, Serialize};

// TODO: add tests for these errors
#[derive(thiserror::Error, Debug, Serialize, Deserialize)]
pub enum Error {
    #[error("The URL of this bookmark is in a format ties doesn't recognize.")]
    InvalidUrl,
    #[error("Ties does not support archiving URLs that point to an IP address.")]
    IpUrl,
    #[error(
        r#"Ties does not support "{scheme}" URLs. URLs starting with "http" or "https" will work."#
    )]
    UnsupportedScheme { scheme: String },
    #[error(r#"Ties does not support websites with a "{content_type}" content type."#)]
    UnsupportedContentType { content_type: String },

    // Reqwest errors
    #[error("Could not reach the website's host.")]
    Connect,
    #[error("The website's host did not answer.")]
    Timeout,
    #[error("The website's host indicated an error on their end (status code: {status:?}).")]
    Status { status: Option<u16> },
    #[error("There was an unexpected error in ties. Please report this to your server operator.")]
    UnexpectedInternal,

    #[error(
        "The size of this website (about {actual_size_mb}MB) is above ties' archive size limit of \
         5MB."
    )]
    ResponseTooLarge { actual_size_mb: f64 },
    #[error("Ties could not convert the website into a readable version.")]
    NotReadable,
}

// Custom From impls are necessary because these libraries don't implement
// Serialize for their errors, so we can't just nest them inside the enum

impl From<url::ParseError> for Error {
    fn from(_value: url::ParseError) -> Self {
        Self::InvalidUrl
    }
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        if value.is_connect() {
            Self::Connect
        } else if value.is_status() {
            Self::Status {
                status: value.status().map(|s| s.as_u16()),
            }
        } else if value.is_timeout() {
            Self::Timeout
        } else {
            tracing::error!(?value, "Encountered unexpected error while archiving");
            Self::UnexpectedInternal
        }
    }
}

impl From<legible::Error> for Error {
    fn from(_value: legible::Error) -> Self {
        Self::NotReadable
    }
}
