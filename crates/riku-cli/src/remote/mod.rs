//! Laptop side list of named remote riku servers, and forwarding to them.
//!
//! `types` holds the validated newtypes and the in-memory list, `store`
//! reads and writes the TOML file that persists it. `git_remote` and
//! `target` pick the server for a command, `forward` runs it over ssh, and
//! `route` ties those together for the binary. `dashboard` opens a server's
//! embedded dashboard in the local browser over an admin ssh session.

pub mod dashboard;
pub mod forward;
pub mod git_remote;
pub mod route;
pub mod server_cmd;
pub mod store;
pub mod target;
pub mod types;

pub use types::{Server, ServerList, ServerName, SshAddress};
