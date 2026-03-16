//! Read facts about the build environment and source version
//! at compile time.

pub fn describe_version() -> String {
    // If git is not installed or the invocation fails during build, this will be
    // `None`.
    option_env!("TIES_VERSION_DESCRIPTION")
        .map(str::to_string)
        .unwrap_or(format!("crate version {}", clap::crate_version!()))
}
