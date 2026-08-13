//! Exclude SQL queries containing a special marker string from logs.
//! Used for hiding queries from the slow query log.
use tracing::field::Visit;
use tracing_subscriber::{
    fmt::{FmtContext, FormatEvent, format::Writer},
    registry::LookupSpan,
};

const EXPECTED_SLOW_MARKER: &str = "ties:expected_slow";

pub(crate) struct SuppressExpectedSlow<F> {
    inner: F,
}

impl<F> SuppressExpectedSlow<F> {
    pub(crate) fn new(inner: F) -> Self {
        Self { inner }
    }
}

struct SuppressVisitor {
    expected_slow: bool,
    is_slow_query_log: bool,
}

impl Visit for SuppressVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "slow_threshold" {
            self.is_slow_query_log = true;
        }
        if value.contains(EXPECTED_SLOW_MARKER) {
            self.expected_slow = true;
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "slow_threshold" {
            self.is_slow_query_log = true;
        }
        if format!("{value:?}").contains(EXPECTED_SLOW_MARKER) {
            self.expected_slow = true;
        }
    }
}

const SQLX_QUERY_TARGET: &str = "sqlx::query";

fn should_suppress(event: &tracing::Event<'_>) -> bool {
    if event.metadata().target() != SQLX_QUERY_TARGET {
        return false;
    }
    let mut visitor = SuppressVisitor {
        expected_slow: false,
        is_slow_query_log: false,
    };
    event.record(&mut visitor);
    visitor.expected_slow && visitor.is_slow_query_log
}

impl<S, N, F> FormatEvent<S, N> for SuppressExpectedSlow<F>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
    F: FormatEvent<S, N>,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        if should_suppress(event) {
            return Ok(());
        }
        self.inner.format_event(ctx, writer, event)
    }
}
