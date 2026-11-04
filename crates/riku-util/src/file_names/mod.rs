//! File names riku derives from an app name joined to another name.
//!
//! App names may hold `.`, `_` and `-`, and Procfile kinds `_` and `-`, so a
//! name joined with `-` or `.` can come out the same for two apps: app `shop`
//! with kind `api-web` and app `shop-api` with kind `web` both gave
//! `shop-api-web-1.toml`. Neither an app name nor a kind can hold `@` (the app
//! name validator drops it, the Procfile parser refuses it), so every derived
//! name puts one `@` right after the app name and splits back at it.

pub mod copy_socket;
pub mod worker;

/// Ends the app name inside a derived file name.
pub const APP_SEPARATOR: char = '@';

/// `name` when it is an app name exactly as the validator returns it.
fn exact_app_name(name: &str) -> Option<&str> {
    if !name.is_empty() && crate::validation::sanitize_app_name(name) == name {
        Some(name)
    } else {
        None
    }
}
