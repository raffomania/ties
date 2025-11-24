//! See <https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html>
//! for information on why our tests are inside the `src` folder.
#![expect(clippy::unwrap_used)]
#![expect(clippy::expect_used)]
mod bookmarks;
mod federation;
mod index;
mod lists;
mod migrations;
mod response_error;
mod search;
mod users;
mod util;
