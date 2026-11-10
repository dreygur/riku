//! Laptop side list of named remote riku servers, and forwarding to them.
//!
//! `types` holds the validated newtypes and the in-memory list, `store`
//! reads and writes the TOML file that persists it. `git_remote` and
//! `target` pick the server for a command, `forward` runs it over ssh, and
//! `route` ties those together for the binary. `dashboard` opens a server's
//! embedded dashboard in the local browser over an admin ssh session.
//! `flow` is the dispatch both binaries run once the target is known.
//! `server_add`, `key_install` and `remote_add` finish `server add`: the
//! deploy key through the admin login and the repo's `riku` remote.

pub mod dashboard;
pub mod flow;
pub mod forward;
pub mod git_remote;
mod key_install;
mod remote_add;
pub mod route;
mod server_add;
pub mod server_cmd;
pub mod store;
pub mod target;
pub mod types;

pub use types::{Server, ServerList, ServerName, SshAddress};
