//! Container services for Riku apps that ship a compose file.
//!
//! - [`types`]: validated names, ports, mounts and lifecycle settings shared
//!   by every other module
//! - [`engine`]: the driver for the Docker compatible CLI
//! - [`compose`]: compose file parsing and validation

pub mod compose;
pub mod engine;
pub mod types;
