//! Supervisor state read back from the files it leaves in the riku root:
//! `stats.json` for workers and the PID file for uptime.

use std::collections::HashMap;
use std::fs;
use std::time::SystemTime;

use crate::config::RikuPaths;
use crate::deploy::supervisor_pid;
use crate::supervisor::stats::AppStats;

/// Load `stats.json` (written periodically by the running supervisor) as
/// `app -> AppStats`, or an empty map if it doesn't exist yet / fails to
/// parse: a dump with no worker data is still useful for inspecting
/// routing and lock state, so this never errors the whole command out.
pub(super) fn load_app_stats(paths: &RikuPaths) -> HashMap<String, AppStats> {
    let stats_file = paths.riku_root.join("stats.json");
    let Ok(content) = fs::read_to_string(&stats_file) else {
        return HashMap::new();
    };
    let Ok(stats_vec) = serde_json::from_str::<Vec<AppStats>>(&content) else {
        return HashMap::new();
    };
    stats_vec.into_iter().map(|s| (s.app.clone(), s)).collect()
}

/// Approximate supervisor uptime from the PID file's mtime, written once,
/// at startup, by `create_pid_file_with_lock`. This command never connects
/// to the running supervisor process, so this is the closest available
/// proxy for "when did it start" without one. `None` unless a supervisor
/// holds the file's lock: one that exited leaves the file behind.
pub(super) fn supervisor_uptime_seconds(paths: &RikuPaths) -> Option<u64> {
    let pid_file = paths.riku_root.join(supervisor_pid::PID_FILE);
    if !supervisor_pid::probe(&pid_file).is_some_and(|probed| probed.locked) {
        return None;
    }
    let Ok(modified) = fs::metadata(&pid_file).and_then(|meta| meta.modified()) else {
        return None;
    };
    SystemTime::now()
        .duration_since(modified)
        .ok()
        .map(|d| d.as_secs())
}

const UPTIME_SKIPPED_AS_ROOT: &str =
    "not read as root, the pid file belongs to the deploy user; run riku __dump-state as that user";

/// Supervisor uptime as the dump reports it.
pub(super) struct SupervisorUptime {
    pub seconds: Option<u64>,
    /// Why `seconds` was not read at all, as opposed to no supervisor
    /// running.
    pub unavailable: Option<&'static str>,
}

/// The uptime, or for root a reason instead: riku-deploy `supervisor_pid`
/// never trusts another user's pid file, so root would otherwise see a
/// stopped supervisor.
pub(super) fn supervisor_uptime(paths: &RikuPaths, as_root: bool) -> SupervisorUptime {
    if as_root {
        return SupervisorUptime {
            seconds: None,
            unavailable: Some(UPTIME_SKIPPED_AS_ROOT),
        };
    }
    SupervisorUptime {
        seconds: supervisor_uptime_seconds(paths),
        unavailable: None,
    }
}

pub(super) fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
