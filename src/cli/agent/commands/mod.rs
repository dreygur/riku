/// Per-command agent implementations.
///
/// Split into:
/// - `cmd_app` — apps, deploy, destroy
/// - `cmd_ops` — config, logs, ps, restart, stop, run, stats

pub(super) mod cmd_app;
pub(super) mod cmd_config;
pub(super) mod cmd_ops;
pub(super) mod cmd_runtime;

pub use cmd_app::{
    cmd_agent_apps, cmd_agent_deploy, cmd_agent_destroy_confirm, cmd_agent_destroy_request,
};
pub use cmd_ops::{
    cmd_agent_config_get, cmd_agent_config_set, cmd_agent_config_show, cmd_agent_logs,
    cmd_agent_ps, cmd_agent_restart, cmd_agent_run, cmd_agent_stats, cmd_agent_stop,
    cmd_agent_stop_confirm,
};
