//! CLI handlers for application management.
//!
//! This module is split into focused sub-modules:
//! - `list`    — app listing
//! - `create`  — app creation
//! - `info`    — app details
//! - `config`  — environment variable management
//! - `deploy`  — deployment from paths and bare repos
//! - `process` — process scaling and status (ps commands)
//! - `logs`    — log tailing
//! - `stats`   — resource usage statistics
//! - `control` — run, restart, stop, update, supervisor, hot-reload

pub mod config;
pub mod control;
pub mod create;
pub mod deploy;
pub mod destroy;
pub mod info;
pub mod list;
pub mod logs;
pub mod process;
pub mod stats;

// Re-export all public functions so callers can use `cli::apps::cmd_*`
pub use config::{cmd_config_get, cmd_config_live, cmd_config_set, cmd_config_show, cmd_config_unset};
pub use control::{cmd_hot_reload, cmd_restart, cmd_run, cmd_stop, cmd_supervisor, cmd_update};
pub use create::cmd_apps_create;
pub use deploy::cmd_deploy;
pub use destroy::cmd_destroy;
pub use info::cmd_apps_info;
pub use list::cmd_apps;
pub use logs::cmd_logs;
pub use process::{cmd_ps_all, cmd_ps_scale, cmd_ps_show};
pub use stats::{cmd_stats_all, cmd_stats_app};
