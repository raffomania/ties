mod fetch_url;
mod queue;
mod readability;
mod safe_dns_resolver;

pub use fetch_url::fetch_url_as_text;
pub use queue::QueueHandle;
pub use readability::make_readable;
