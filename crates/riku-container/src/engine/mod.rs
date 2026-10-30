//! Driver for the Docker compatible container engine CLI.
//!
//! # Security model
//!
//! Every value reaches the engine as a separate argv element, never through a
//! shell. Names, paths and image references that came from a compose file are
//! validated by the types in [`crate::types`] before they are used here.

mod detect;
#[cfg(test)]
mod fake;
mod identity;
mod probe;

pub use detect::{detect, EngineBinary, KNOWN_ENGINES};
pub use identity::{parse_identity, Engine, EngineIdentity};
pub use probe::REQUIRED_COMMANDS;
