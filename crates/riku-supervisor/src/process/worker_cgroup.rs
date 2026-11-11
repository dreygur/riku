//! The cgroup an isolated worker runs in, provisioned under the cgroup the
//! supervisor took over at startup.

use anyhow::{anyhow, Result};

use super::ProcessManager;
use crate::cgroups::{CgroupLimits, CgroupTree, WorkerCgroup};
use crate::config::WorkerConfig;

impl ProcessManager {
    /// Use `tree` for the cgroups of isolated workers. Without it, a worker
    /// that asks for isolation refuses to start.
    pub fn set_cgroup_tree(&mut self, tree: CgroupTree) {
        self.cgroup_tree = Some(tree);
    }

    /// The provisioned cgroup for `config`, or `None` when the worker does
    /// not ask for isolation.
    pub(super) fn worker_cgroup(&self, config: &WorkerConfig) -> Result<Option<WorkerCgroup>> {
        let Some(opts) = &config.options.isolation else {
            return Ok(None);
        };
        let worker = config.worker.name();
        let Some(tree) = &self.cgroup_tree else {
            return Err(anyhow!(
                "{worker} asks for isolation, but the supervisor has no delegated cgroup; \
                 see the cgroup diagnostic in the supervisor log"
            ));
        };
        let limits = CgroupLimits {
            memory_max_bytes: opts.max_memory_bytes,
            cpu_quota_us: opts.cpu_quota_us,
            cpu_period_us: opts.cpu_period_us,
            pids_max: opts.max_pids,
        };
        match WorkerCgroup::provision(tree, &worker, &limits) {
            Ok(cgroup) => Ok(Some(cgroup)),
            Err(e) => Err(e),
        }
    }
}
