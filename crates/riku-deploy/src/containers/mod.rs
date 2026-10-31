//! Turning compose services into running container copies.
//!
//! # Security model
//!
//! App names, compose files and app env all come from users. [`AppName`] can
//! only be built from a name the engine accepts inside a container name and a
//! label filter. [`start_copy`] repeats the bind source symlink walk right
//! before the engine runs, because the checkout can change after parsing, and
//! refuses a container name another app already holds. Secrets (registry
//! token, env values) are never logged and the token only travels on the
//! engine's stdin.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

mod images;
mod ports;
mod spec;
mod start;

pub use images::prepare_images;
pub use ports::pick_free_port;
pub use spec::copy_spec;
pub use start::start_copy;

use riku_container::types::check_app_name;

/// An app name that is safe as part of a container, network and image name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppName(String);

impl AppName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for AppName {
    type Error = anyhow::Error;

    fn try_from(raw: &str) -> anyhow::Result<Self> {
        match check_app_name(raw) {
            Ok(()) => Ok(AppName(raw.to_string())),
            Err(e) => Err(e),
        }
    }
}

impl std::fmt::Display for AppName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
