//! Compose file parsing and validation.
//!
//! # Security model
//!
//! The compose file is app code from a git push, so it is untrusted input.
//! Every value is validated before it reaches the engine, and any key this
//! module does not support is a deploy error.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

mod build;
mod definition;
mod depends;
mod dotenv_value;
mod duration;
mod environment;
mod fields;
mod health;
mod interpolate;
mod keys;
mod locate;
mod mounts;
mod order;
mod paths;
mod ports;
mod project;
mod project_name;
mod raw;
mod rules;
mod scalar;
mod service;
mod service_env;
mod service_steps;
#[cfg(test)]
mod test_support;
mod volume_decl;
mod volumes;

pub use definition::{BuildDef, Dependency, DependsCondition, ServiceContext, ServiceDef};
pub use locate::{default_compose_file, find_compose_file, ComposeFile, COMPOSE_FILE_NAMES};
pub use mounts::{ServiceMount, ServiceMountSource, VolumeKey};
pub use paths::check_bind_source;
pub use project::{
    existing_volume_name, read_project, single_image_project, ComposeProject, ProjectNameSource,
    VolumeDecl,
};
pub use raw::{read_raw, RawProject};
pub use rules::{check_scale, replace_order, routed_http, ReplaceOrder, RoutedPort};
pub use serde_json::Value;
pub use service::parse_service;

/// A YAML mapping with string keys, as held inside a [`Value`].
pub type Mapping = serde_json::Map<String, Value>;
