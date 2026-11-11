//! How `[options]` is read from a worker TOML, including files written
//! before the limits were split from the sandbox.
//!
//! Security model: those older files kept the cgroup limits inside
//! `[options.isolation]`, next to `root_dir`. The supervisor re-reads
//! `workers-enabled/` at startup, before any deploy rewrites the files, so
//! dropping those fields would restart a sandboxed worker without its
//! limits. They are read here into `limits` when `[options.limits]` is
//! absent, and never written back.

use serde::Deserialize;

use super::{
    default_grace_period, default_max_restarts, default_timeout, HealthCheck, Sandbox,
    WorkerLimits, WorkerOptions,
};

/// `[options]` exactly as it may appear on disk.
#[derive(Deserialize)]
pub(super) struct OptionsFile {
    working_dir: String,
    log_file: String,
    #[serde(default)]
    uid: Option<String>,
    #[serde(default)]
    gid: Option<String>,
    #[serde(default = "default_timeout")]
    timeout: u64,
    #[serde(default = "default_grace_period")]
    grace_period: u64,
    #[serde(default = "default_max_restarts")]
    max_restarts: u32,
    #[serde(default)]
    health_check: Option<HealthCheck>,
    #[serde(default)]
    limits: Option<WorkerLimits>,
    #[serde(default)]
    isolation: Option<IsolationSection>,
}

/// `[options.isolation]`: the sandbox root, plus the limits an older file
/// kept here.
#[derive(Deserialize)]
struct IsolationSection {
    root_dir: String,
    #[serde(flatten)]
    legacy_limits: WorkerLimits,
}

impl From<OptionsFile> for WorkerOptions {
    fn from(file: OptionsFile) -> Self {
        let (sandbox, legacy_limits) = match file.isolation {
            Some(section) => (
                Some(Sandbox {
                    root_dir: section.root_dir,
                }),
                section
                    .legacy_limits
                    .limits_something()
                    .then_some(section.legacy_limits),
            ),
            None => (None, None),
        };
        WorkerOptions {
            working_dir: file.working_dir,
            log_file: file.log_file,
            uid: file.uid,
            gid: file.gid,
            timeout: file.timeout,
            grace_period: file.grace_period,
            max_restarts: file.max_restarts,
            health_check: file.health_check,
            limits: file.limits.or(legacy_limits),
            sandbox,
        }
    }
}
