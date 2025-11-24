/// Used to unescape html returned from the server, e.g. when extracting URLs or
/// other content.
pub fn html_decode(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", r#"""#)
        .replace("&#x27;", "'")
}
