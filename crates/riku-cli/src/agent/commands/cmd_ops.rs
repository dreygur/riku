//! Agent commands for operations: config, logs, ps, restart, stop, run, stats.
//!
//! Split into:
//! - `cmd_config`: config:get, config:show
//! - `cmd_config_write`: config:set, config:unset
//! - `cmd_runtime`: logs, ps, restart, stop, stats
//! - `cmd_run`: run

pub use super::cmd_config::{cmd_agent_config_get, cmd_agent_config_show};
pub use super::cmd_config_write::{cmd_agent_config_set, cmd_agent_config_unset};
pub use super::cmd_run::cmd_agent_run;
pub use super::cmd_runtime::{
    cmd_agent_logs, cmd_agent_ps, cmd_agent_restart, cmd_agent_stats, cmd_agent_stop,
    cmd_agent_stop_confirm,
};
