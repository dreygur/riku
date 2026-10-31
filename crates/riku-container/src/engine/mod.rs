//! Driver for the Docker compatible container engine CLI.
//!
//! Riku needs Docker 23 or newer, or an engine whose CLI accepts the same
//! commands: `version` and `stats` are read with `--format json`, which older
//! Docker releases do not support.
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
mod inspect_parse;
mod login;
mod lookup;
mod probe;
mod resources;
mod run_args;
mod run_spec;
mod snapshot;
mod spec_check;
mod stats;

pub use build::BuildSpec;
pub use detect::{detect, EngineBinary, KNOWN_ENGINES};
pub use identity::{parse_identity, Engine, EngineIdentity};
pub use ids::ContainerId;
pub use inspect::{ContainerInfo, ContainerState};
pub use probe::REQUIRED_COMMANDS;
pub use run_spec::RunSpec;
pub use stats::ContainerStats;
