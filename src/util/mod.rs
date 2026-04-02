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

pub mod display;
pub mod env;
pub mod nginx_validation;
pub mod procfile;
pub mod process_util;
pub mod ssh_keys;
pub mod validation;

// Re-export everything so existing call-sites (`crate::util::echo`, etc.) keep working.
pub use display::{echo, format_table, print_table, print_table_with_title};
pub use env::{expandvars, parse_settings, write_config};
pub use ssh_keys::setup_authorized_keys;
pub use nginx_validation::{print_env_warnings, validate_env_vars, validate_nginx_cache_config};
pub use procfile::parse_procfile;
pub use process_util::{
    check_requirements, command_output, found_app, get_free_port, validate_node_version,
};
pub use validation::{
    ensure_path_within, exit_if_invalid, get_boolean, parse_positive_int, sanitize_app_name,
    validate_app_name,
};
