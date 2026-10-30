//! Compose file parsing and validation.
//!
//! # Security model
//!
//! The compose file is app code from a git push, so it is untrusted input.
//! Every value is validated before it reaches the engine, and any key this
//! module does not support is a deploy error.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

mod environment;
mod interpolate;
mod keys;
mod locate;
mod raw;

pub use locate::{default_compose_file, find_compose_file, ComposeFile, COMPOSE_FILE_NAMES};
pub use raw::{read_raw, RawProject};
pub use serde_json::Value;

/// A YAML mapping with string keys, as held inside a [`Value`].
pub type Mapping = serde_json::Map<String, Value>;
