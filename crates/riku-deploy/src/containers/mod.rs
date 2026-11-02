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

mod app;
mod context;
mod control;
mod copy_restart;
mod deploy;
mod engine_open;
mod image_update;
mod images;
mod legacy;
mod legacy_workers;
mod migrate;
#[cfg(test)]
mod mock_legacy;
#[cfg(test)]
pub(crate) mod mock_state;
mod ports;
mod project_check;
mod release;
mod removal;
mod rollout;
mod routes;
mod runtime;
mod spec;
mod start;

pub use app::{is_container_app, load_project, read_scaling, SINGLE_IMAGE_PORT};
pub use control::{restart_service, runs_containers, scale_service, stop_service};
pub use copy_restart::{restart_copy, RestartOutcome};
pub use deploy::{deploy_containers, CONTAINER_RUNTIME};
pub use engine_open::{open_engine, EngineUnreachable};
pub use image_update::{update_service, ImageUpdate};
pub use images::prepare_images;
pub use legacy::{is_legacy_worker, legacy_apps};
pub use ports::pick_free_port;
pub use project_check::confirm_project_name;
pub use removal::destroy_containers;
pub use riku_container::types::CopyIndex;
pub use riku_container::types::ServiceName;
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

#[cfg(test)]
#[path = "unreachable_tests.rs"]
mod unreachable_tests;
