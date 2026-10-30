//! Driver for the Docker compatible container engine CLI.
//!
//! # Security model
//!
//! Every value reaches the engine as a separate argv element, never through a
//! shell. Names, paths and image references that came from a compose file are
//! validated by the types in [`crate::types`] before they are used here.

mod build;
mod control;
mod detect;
mod env_file;
#[cfg(test)]
mod fake;
#[cfg(test)]
mod fixture;
mod identity;
mod ids;
mod inspect;
mod probe;
mod resources;
mod run_args;
mod spec_check;
mod stats;

pub use build::BuildSpec;
pub use detect::{detect, EngineBinary, KNOWN_ENGINES};
pub use identity::{parse_identity, Engine, EngineIdentity};
pub use ids::ContainerId;
pub use inspect::{ContainerInfo, ContainerState};
pub use probe::REQUIRED_COMMANDS;
pub use run_args::RunSpec;
pub use stats::ContainerStats;
