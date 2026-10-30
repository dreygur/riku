//! Setup commands: server initialization, binary installation, systemd, and SSH key management.

pub mod binary;
mod caddy_host;
mod default_branch;
mod deploy_account;
mod deploy_keys;
pub(crate) mod deploy_user;
pub mod git_hook;
mod guidance;
pub mod init;
mod init_router;
mod prerequisites;
mod root_init;
mod router_units;
pub mod ssh;
pub mod system_service;
pub mod systemd;
pub mod user_service;

pub use init::{cmd_init, InitOptions};
