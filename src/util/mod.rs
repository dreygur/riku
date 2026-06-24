//! Utility functions for Riku.
//!
//! Organised into focused sub-modules:
//! - [`display`]           — terminal output and table formatting
//! - [`validation`]        — app-name sanitization, path-traversal checks
//! - [`nginx_validation`]  — nginx cache config and env-var validation
//! - [`procfile`]          — Procfile parsing and cron validation
//! - [`env`]               — KEY=VALUE file parsing, variable expansion
//! - [`ssh_keys`]          — SSH authorized_keys management
//! - [`process_util`]      — free-port allocation, shell command helpers, binary checks

pub mod deploy_logger;
pub mod display;
pub mod env;
pub mod fs;
pub mod nginx_validation;
pub mod process_util;
pub mod procfile;
pub mod ssh_keys;
pub mod validation;

// Re-export items used via `crate::util::*` call-sites.
pub use display::{echo, print_table, print_table_with_title};
pub use env::{parse_settings, write_config};
pub use fs::{copy_dir_recursive, count_files, write_atomic};
pub use nginx_validation::{print_env_warnings, validate_env_vars};
pub use process_util::get_free_port;
pub use procfile::parse_procfile;
pub use ssh_keys::setup_authorized_keys;
pub use validation::{ensure_path_within, exit_if_invalid, validate_app_name};
