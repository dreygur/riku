//! Utility functions for Riku.
//!
//! Organised into focused sub-modules:
//! - [`backend`]: where a router sends an app's traffic
//! - [`route_plan`]: domains, HTTPS policy and backends handed to a router
//! - [`spawn`]: process spawning that tolerates the transient ETXTBSY race
//! - [`display`]: terminal output and table formatting
//! - [`domain`]: validated hostnames for router config
//! - [`validation`]: app-name sanitization, path-traversal checks
//! - [`file_names`]: worker config and copy socket names built from an app name
//! - [`nginx_validation`]: warnings for nginx related env vars
//! - [`procfile`]: Procfile parsing and cron validation
//! - [`env`]: KEY=VALUE file parsing, variable expansion
//! - [`authorized_keys`]: migration of old deploy key forced commands
//! - [`ssh_keys`]: SSH authorized_keys management
//! - [`process_util`]: free-port allocation, Node version validation

pub mod authorized_keys;
pub mod backend;
pub mod cron;
pub mod deploy_logger;
pub mod display;
pub mod domain;
pub mod env;
pub mod file_names;
pub mod flag;
pub mod fs;
pub mod listening;
pub mod log_stamp;
pub mod nginx_validation;
pub mod outbound_ip;
pub mod process_util;
pub mod procfile;
pub mod public_key;
pub mod resource_limits;
pub mod root_binary;
pub mod route_plan;
pub mod runtime_markers;
pub mod secure;
pub mod socket_dir;
pub mod spawn;
pub mod ssh_keys;
pub mod token;
pub mod validation;
pub mod write_dir;

// Re-export items used via `crate::util::*` call-sites.
pub use display::{print_table, print_table_with_title};
pub use env::{parse_settings, write_config};
pub use fs::{
    copy_dir_recursive, count_files, create_private_dir, restrict_to_owner, write_atomic,
    write_private,
};
pub use nginx_validation::{print_env_warnings, validate_env_vars};
pub use process_util::get_free_port;
pub use procfile::parse_procfile;
pub use root_binary::ROOT_BINARY;
pub use ssh_keys::{setup_authorized_keys, setup_authorized_keys_in};
pub use validation::{ensure_path_within, existing_app, exit_if_invalid, validate_app_name};
