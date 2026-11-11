//! Setup commands: server initialization, binary installation, systemd, and SSH key management.

pub mod binary;
mod caddy_admin;
mod caddy_host;
mod default_branch;
mod deploy_account;
mod deploy_key_cmd;
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
mod runtime_plugins;
pub(crate) mod server_ip;
mod socket_group;
pub mod ssh;
mod stale_binary;
mod supervisor_binary;
mod system_install;
pub mod system_service;
pub mod systemd;
pub mod user_service;
mod var_root;

pub use deploy_key_cmd::cmd_deploy_key_add;
pub use init::{cmd_init, InitOptions};
