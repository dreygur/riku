//! Laptop side list of named remote riku servers.
//!
//! `types` holds the validated newtypes and the in-memory list, `store`
//! reads and writes the TOML file that persists it.

pub mod store;
pub mod types;

pub use types::{Server, ServerList, ServerName, SshAddress};
