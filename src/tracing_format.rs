use core::fmt;

use tracing::Subscriber;
use tracing_subscriber::{
    fmt::{
        FmtContext, FormatEvent,
        format::{Format, Pretty, Writer},
    },
    registry::LookupSpan,
};

/// [tracing_subscriber] format used for verbose, easy-to-read logs in
/// development.
pub(crate) struct DevelopmentFormat {
    inner: Format<Pretty>,
}

impl DevelopmentFormat {
    pub(crate) fn new() -> Self {
        Self {
            inner: Format::default()
                .pretty()
                // Always use colors.
                .with_ansi(true)
                // Reduce noise.
                .with_source_location(false),
        }
    }
}

impl<S, N> FormatEvent<S, N> for DevelopmentFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> fmt::Result {
        let mut unescaping = UnescapingWriter {
            inner: writer.by_ref(),
            pending_backslashes: false,
        };
        self.inner
            .format_event(ctx, Writer::new(&mut unescaping), event)?;
        unescaping.flush_remaining()
    }
}

/// Replace '\\n' with '\n' to actually print newlines on the console.
struct UnescapingWriter<'a> {
    inner: Writer<'a>,
    pending_backslashes: bool,
}

impl UnescapingWriter<'_> {
    fn flush_remaining(&mut self) -> fmt::Result {
        if self.pending_backslashes {
            self.inner.write_str("\\")?;
        }
        Ok(())
    }
}

impl fmt::Write for UnescapingWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let chars = s.chars();
        for c in chars {
            if self.pending_backslashes {
                self.pending_backslashes = false;
                if c == 'n' {
                    self.inner.write_char('\n')?;
                } else {
                    self.inner.write_char('\\')?;
                    self.inner.write_char(c)?;
                }
            } else if c == '\\' {
                self.pending_backslashes = true;
            } else {
                self.inner.write_char(c)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    fn unescape(input: &str) -> String {
        let mut buf = String::new();
        {
            let mut writer = Writer::new(&mut buf);
            let mut unescaping = UnescapingWriter {
                inner: writer.by_ref(),
                pending_backslashes: false,
            };
            unescaping.write_str(input).unwrap();
            unescaping.flush_remaining().unwrap();
        }
        buf
    }

    #[test]
    fn newline_unescaped() {
        assert_eq!(unescape("\\n"), "\n");
    }

    #[test]
    fn double_backslash_passes_through() {
        assert_eq!(unescape("\\\\"), "\\\\");
    }

    #[test]
    fn double_backslash_then_n_passes_through() {
        assert_eq!(unescape("\\\\n"), "\\\\n");
    }

    #[test]
    fn tab_escape_passes_through() {
        assert_eq!(unescape("\\t"), "\\t");
    }

    #[test]
    fn trailing_backslash_preserved() {
        assert_eq!(unescape("\\"), "\\");
    }

    #[test]
    fn empty_input() {
        assert_eq!(unescape(""), "");
    }

    #[test]
    fn no_escapes() {
        assert_eq!(unescape("hello world"), "hello world");
    }

    #[test]
    fn multiple_newlines() {
        assert_eq!(unescape("line1\\nline2\\nline3"), "line1\nline2\nline3");
    }

    #[test]
    fn actual_newline_passes_through() {
        assert_eq!(unescape("hello\nworld"), "hello\nworld");
    }

    #[test]
    fn mixed_escapes() {
        assert_eq!(unescape("a\\nb\\\\c\\nd"), "a\nb\\\\c\nd");
    }
}
