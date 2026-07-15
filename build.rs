use std::{env, fs, path::Path, process::Command};

use railwind::{Source, SourceOptions};
use regex::Regex;

fn git_cmd(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Reorders CSS so that all `@media (prefers-color-scheme: dark)` blocks appear
/// at the end.
///
/// This ensures dark mode styles have higher specificity than light mode
/// styles, since CSS rules with equal specificity are resolved by source order.
/// Once we switch to encre, check if they fix this for us or if we should
/// contribute a fix upstream.
fn reorder_dark_mode_css(css: &str) -> String {
    let mut normal_rules = String::new();
    let mut dark_blocks = String::new();

    let mut in_dark_block = false;
    let mut brace_depth: u32 = 0;
    let mut current_block = String::new();

    for line in css.lines() {
        if in_dark_block {
            current_block.push_str(line);
            current_block.push('\n');

            // Track brace depth to find end of block
            for ch in line.chars() {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => {
                        brace_depth -= 1;
                    }
                    _ => {}
                }

                if brace_depth == 0 {
                    // End of dark mode block
                    dark_blocks.push_str(&current_block);
                    current_block.clear();
                    in_dark_block = false;
                    break;
                }
            }
        } else if line.starts_with("@media (prefers-color-scheme: dark)") {
            // Start of dark mode block
            in_dark_block = true;
            brace_depth = 0;
            current_block.push_str(line);
            current_block.push('\n');

            // Count opening brace on the same line
            for ch in line.chars() {
                if ch == '{' {
                    brace_depth += 1;
                }
            }
        } else {
            normal_rules.push_str(line);
            normal_rules.push('\n');
        }
    }

    normal_rules.push_str(&dark_blocks);
    normal_rules
}

#[expect(clippy::unwrap_used, reason = "Panicking at compile time is fine")]
#[expect(clippy::expect_used, reason = "Panicking at compile time is fine")]
fn main() {
    // Without this, adding only a migration will not trigger a re-build
    // https://docs.rs/sqlx/latest/sqlx/macro.migrate.html#stable-rust-cargo-build-script
    println!("cargo:rerun-if-changed=migrations");

    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-env-changed=TIES_VERSION_DESCRIPTION");

    let version_description_exists = env::var("TIES_VERSION_DESCRIPTION").is_ok();
    if !version_description_exists
        && let Some(describe) = git_cmd(&["describe", "--tags", "--long", "--dirty"])
    {
        println!("cargo:rustc-env=TIES_VERSION_DESCRIPTION={describe}");
    }

    println!("cargo:rerun-if-changed=src/views");

    let out_dir = env::var("OUT_DIR").unwrap();

    let dest_path = Path::new(&out_dir).join("railwind.css");

    let paths: Vec<_> = walkdir::WalkDir::new("src/views")
        .into_iter()
        .map(|e| e.expect("Error while searching for views"))
        .filter(|e| !e.file_type().is_dir())
        .map(walkdir::DirEntry::into_path)
        .collect();

    let sources = paths
        .iter()
        .map(|p| SourceOptions {
            input: p,
            option: railwind::CollectionOptions::Regex(
                Regex::new(r#"class[\n\s\(]*"([^"]+)""#).unwrap(),
            ),
        })
        .collect();

    let source = Source::Files(sources);
    let css = railwind::parse_to_string(source, false, &mut Vec::new());
    let reordered = reorder_dark_mode_css(&css);
    fs::write(dest_path, reordered).unwrap();
}
