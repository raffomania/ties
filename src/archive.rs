mod error;
mod fetch_url;
mod readability;
mod safe_ips;

pub use error::Error;
pub use fetch_url::fetch_url_as_text;
pub use readability::make_readable;
