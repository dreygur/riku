//! Agent commands for operations: config, logs, ps, restart, stop, run, stats.
//!
//! Split into:
//! - `cmd_config` — config:get, config:set, config:show
//! - `cmd_runtime` — logs, ps, restart, stop, run, stats

pub use super::cmd_config::{cmd_agent_config_get, cmd_agent_config_set, cmd_agent_config_show};
pub use super::cmd_runtime::{
    cmd_agent_logs, cmd_agent_ps, cmd_agent_restart, cmd_agent_run, cmd_agent_stats,
    cmd_agent_stop, cmd_agent_stop_confirm,
};
