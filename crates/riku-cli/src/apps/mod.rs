//! CLI handlers for application management.
//!
//! This module is split into focused sub-modules:
//! - `list`: app listing
//! - `create`: app creation
//! - `info`: app details
//! - `config`: environment variable management
//! - `deploy`: deployment from paths and bare repos
//! - `process`: process scaling and status (ps commands)
//! - `logs`: log tailing
//! - `stats`: resource usage statistics
//! - `control`: run, restart, stop, update, supervisor
//! - `hot_reload`: restart --hot
//! - `control_target`: container app or Procfile app, for per service commands

mod certs_cmd;
mod certs_reapply;
pub mod config;
pub mod control;
mod control_target;
pub mod create;
pub mod deploy;
pub mod destroy;
mod destroy_engine;
mod destroy_files;
pub mod dump_state;
mod embedded_plugins;
pub mod hot_reload;
pub mod info;
pub mod install_plugins;
pub mod list;
pub mod logs;
mod plugin_download;
pub mod plugin_emit;
mod plugin_write;
pub mod process;
pub mod rollback;
pub mod settings_cmd;
pub mod stats;

// Re-export all public functions so callers can use `cli::apps::cmd_*`
pub use certs_cmd::{cmd_certs_set, cmd_certs_show, cmd_certs_unset};
pub use config::{
    cmd_config_get, cmd_config_live, cmd_config_set, cmd_config_show, cmd_config_unset,
    config_after_set, config_after_unset,
};
pub use control::{cmd_restart, cmd_run, cmd_stop, cmd_supervisor};
pub use create::cmd_apps_create;
pub use deploy::cmd_deploy;
pub use destroy::{cmd_destroy, DestroyMode};
pub use dump_state::{cmd_dump_state, state_json};
pub use hot_reload::cmd_hot_reload;
pub use info::cmd_apps_info;
pub use install_plugins::cmd_install_plugins;
pub use list::cmd_apps;
pub use logs::{cmd_deploy_logs, cmd_logs};
pub use plugin_emit::cmd_plugin_emit;
pub use process::{cmd_ps_all, cmd_ps_scale, cmd_ps_show};
pub use rollback::cmd_rollback;
pub use settings_cmd::{cmd_settings_get, cmd_settings_set, cmd_settings_show, cmd_settings_unset};
pub use stats::{cmd_stats_all, cmd_stats_app};
