//! The unix socket one uWSGI copy of an app listens on, `<app>@<n>.sock` in
//! the router's socket directory.
//!
//! The `@` keeps it apart from the single `<app>.sock` of an app named
//! `<app>.<n>`, which a `<app>.<n>.sock` copy socket would equal.

use super::{exact_app_name, APP_SEPARATOR};

const SOCKET_EXTENSION: &str = ".sock";

/// `<app>@<copy>.sock`.
pub fn file_name(app: &str, copy: u32) -> String {
    format!("{app}{APP_SEPARATOR}{copy}{SOCKET_EXTENSION}")
}

/// The app and copy number a `<app>@<n>.sock` file name holds.
#[allow(clippy::question_mark)]
pub fn parse(file_name: &str) -> Option<(&str, u32)> {
    let Some(stem) = file_name.strip_suffix(SOCKET_EXTENSION) else {
        return None;
    };
    let Some((app, copy)) = stem.split_once(APP_SEPARATOR) else {
        return None;
    };
    match (exact_app_name(app), copy_number(copy)) {
        (Some(app), Some(copy)) => Some((app, copy)),
        _ => None,
    }
}

fn copy_number(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
#[path = "copy_socket_tests.rs"]
mod tests;
