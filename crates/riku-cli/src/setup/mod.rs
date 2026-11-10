//! Setup commands: server initialization, binary installation, systemd, and SSH key management.

pub mod binary;
mod caddy_admin;
mod caddy_host;
mod default_branch;
mod deploy_account;
mod deploy_keys;
pub(crate) mod deploy_user;
pub mod git_hook;
mod guidance;
mod helper_install;
mod host_cleanup;
pub mod init;
mod init_router;
mod nginx_host;
mod nginx_worker;
mod prerequisites;
mod root_init;
mod router_choice;
mod router_units;
mod socket_group;
pub mod ssh;
mod stale_binary;
pub mod system_service;
pub mod systemd;
pub mod user_service;
mod var_root;

pub use init::{cmd_init, InitOptions};
