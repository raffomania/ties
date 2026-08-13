use uuid::Uuid;

use crate::db::LinkDestination;

#[must_use]
pub fn bookmark(bookmark_id: Uuid) -> String {
    format!("/bookmarks/{bookmark_id}")
}

#[must_use]
pub fn edit_bookmark(bookmark_id: Uuid) -> String {
    format!("/bookmarks/{bookmark_id}/edit")
}

#[must_use]
pub fn list(list_id: Uuid) -> String {
    format!("/lists/{list_id}")
}

#[must_use]
pub fn list_feed(list_id: Uuid) -> String {
    format!("/lists/{list_id}/feed.atom")
}

#[must_use]
pub fn local_user(username: &str) -> String {
    format!("/user/{username}")
}

#[must_use]
pub fn local_user_feed(username: &str) -> String {
    format!("/user/{username}/feed.atom")
}

#[must_use]
pub fn link_destination(destination: &LinkDestination) -> String {
    match destination {
        LinkDestination::Bookmark(b) => bookmark(b.id),
        LinkDestination::List(l) => list(l.id),
    }
}

pub fn explore_list(list_id: Uuid) -> String {
    format!("/lists/{list_id}/explore")
}
