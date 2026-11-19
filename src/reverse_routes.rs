#[must_use]
pub fn local_user_path(username: &str) -> String {
    format!("/user/{username}")
}

#[must_use]
pub fn local_user_feed_path(username: &str) -> String {
    format!("/user/{username}/feed.atom")
}
