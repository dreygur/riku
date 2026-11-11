//! The cgroup a worker runs in when it sets a resource limit or is
//! sandboxed, provisioned under the cgroup the supervisor took over at
//! startup.

use anyhow::{anyhow, Result};

use super::ProcessManager;
use crate::cgroups::{CgroupLimits, CgroupTree, WorkerCgroup};
use crate::config::{WorkerConfig, WorkerLimits};

impl From<&WorkerLimits> for CgroupLimits {
    fn from(limits: &WorkerLimits) -> Self {
        CgroupLimits {
            memory_max_bytes: limits.max_memory_bytes,
            cpu_quota_us: limits.cpu_quota_us,
            cpu_period_us: limits.cpu_period_us,
            pids_max: limits.max_pids,
        }
    }
}

impl ProcessManager {
    /// Use `tree` for worker cgroups. Without it, a worker that sets a
    /// resource limit or a sandbox refuses to start.
    pub fn set_cgroup_tree(&mut self, tree: CgroupTree) {
        self.cgroup_tree = Some(tree);
    }

    /// The provisioned cgroup for `config`, or `None` when the worker sets
    /// no resource limit and no sandbox. A sandboxed worker without limits
    /// still gets a cgroup, so stopping it reaches every process in it.
    pub(super) fn worker_cgroup(&self, config: &WorkerConfig) -> Result<Option<WorkerCgroup>> {
        let options = &config.options;
        let limits = match (&options.limits, &options.sandbox) {
            (None, None) => return Ok(None),
            (Some(limits), _) => CgroupLimits::from(limits),
            (None, Some(_)) => CgroupLimits::default(),
        };
        let worker = config.worker.name();
        let Some(tree) = &self.cgroup_tree else {
            return Err(anyhow!(
                "{worker} sets a resource limit or a sandbox, but the supervisor has no \
                 delegated cgroup; see the cgroup diagnostic in the supervisor log"
            ));
        };
        match WorkerCgroup::provision(tree, &worker, &limits) {
            Ok(cgroup) => Ok(Some(cgroup)),
            Err(e) => Err(e),
        }
    }
}
