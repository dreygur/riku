//! Compose file parsing and validation.
//!
//! # Security model
//!
//! The compose file is app code from a git push, so it is untrusted input.
//! Every value is validated before it reaches the engine, and any key this
//! module does not support is a deploy error.

mod locate;

pub use locate::{default_compose_file, find_compose_file, ComposeFile, COMPOSE_FILE_NAMES};
