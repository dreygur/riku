//! cgroups v2 resource constraints for spawned worker processes.
//!
//! Provisions one cgroup per isolated worker under the supervisor's own
//! delegated cgroup (see [`tree`]) and writes `memory.max`, `cpu.max` and
//! `pids.max` before the worker starts running, so it is constrained from
//! its very first instruction.
//!
//! Security model: only the supervisor, running as the deploy user, writes
//! here, and only inside the subtree systemd delegated to its unit. Nothing
//! runs as root, and nothing outside that subtree is touched. A worker's
//! cgroup directory is named after its [`WorkerName`], which is checked to
//! be a well formed `<app>@<kind>.<n>` before it becomes a path, so a
//! config cannot point a cgroup outside `apps/`.

use anyhow::{anyhow, Context, Result};
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use riku_util::file_names::worker::WorkerName;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

pub mod tree;

pub use tree::{take_over_own, CgroupTree};

#[cfg(test)]
mod tests;

/// How long a killed cgroup gets to empty before its removal is tried.
const KILL_SETTLE: Duration = Duration::from_secs(1);

/// CPU/memory constraints for a single worker's cgroup.
#[derive(Debug, Clone, Default)]
pub struct CgroupLimits {
    /// Hard memory ceiling in bytes, written to `memory.max`. `None` leaves
    /// the kernel default (`max`, i.e. unlimited).
    pub memory_max_bytes: Option<u64>,
    /// CPU quota in microseconds per `cpu_period_us`, written to `cpu.max`
    /// as `"<quota> <period>"`. `None` leaves `cpu.max` alone (unlimited)
    /// and so needs no cpu controller.
    pub cpu_quota_us: Option<u64>,
    /// CPU accounting period in microseconds (kernel default and most
    /// common choice is 100000, i.e. 100ms).
    pub cpu_period_us: u64,
    /// Maximum number of tasks (processes/threads) this worker's cgroup may
    /// ever contain at once, written to `pids.max`. `None` leaves the
    /// kernel default (`max`, i.e. unlimited).
    ///
    /// This is the correct way to bound a worker's fork count: unlike
    /// `RLIMIT_NPROC` (see `resource_limits::ResourceLimits`), which counts
    /// every process owned by the real UID system-wide and so falsely
    /// trips on an otherwise-healthy worker whenever anything else under
    /// that UID is also running, `pids.max` is scoped to exactly this
    /// cgroup's subtree.
    pub pids_max: Option<u64>,
}

/// One limit to write: the controller that owns it, its file, its value.
struct LimitNode {
    controller: &'static str,
    file: &'static str,
    value: String,
}

impl CgroupLimits {
    /// The files these limits ask for. A limit left at `None` writes none.
    fn nodes(&self) -> Vec<LimitNode> {
        let memory = self.memory_max_bytes.map(|bytes| LimitNode {
            controller: "memory",
            file: "memory.max",
            value: bytes.to_string(),
        });
        let cpu = self.cpu_quota_us.map(|quota| LimitNode {
            controller: "cpu",
            file: "cpu.max",
            value: format!("{quota} {}", self.cpu_period_us),
        });
        let pids = self.pids_max.map(|pids| LimitNode {
            controller: "pids",
            file: "pids.max",
            value: pids.to_string(),
        });
        [memory, cpu, pids].into_iter().flatten().collect()
    }
}

/// A provisioned cgroup for one worker process. Cheap to clone (just the
/// path) so a handle can be moved into a `pre_exec` closure while another
/// stays with the supervisor for later polling/cleanup.
#[derive(Clone)]
pub struct WorkerCgroup {
    path: PathBuf,
}

/// Render a structured startup diagnostic for a failed [`take_over_own`]:
/// what failed, why it matters, and how to fix it, so the operator sees it
/// at boot instead of the first time a worker asks for isolation.
pub fn startup_diagnostic(err: &anyhow::Error) -> String {
    format!(
        "riku startup diagnostic\n\
         \x20 component : cgroup v2 resource isolation\n\
         \x20 check     : write access to the supervisor's own cgroup (from /proc/self/cgroup)\n\
         \x20 status    : FAILED\n\
         \x20 detail    : {err:#}\n\
         \x20 impact    : any worker with `isolation` enabled in its config refuses to start\n\
         \x20 remedy    : add Delegate=yes to the riku unit (re-run riku init to rewrite it)\n\
         \x20             and restart it; cgroup v2 must be mounted at {mount}",
        mount = tree::CGROUP_MOUNT,
    )
}

impl WorkerCgroup {
    /// Create (or reuse) the cgroup directory for `worker` under the
    /// supervisor's `apps/` cgroup and write `limits` to it. Call this from
    /// the supervisor (parent) before spawning the worker, so the
    /// constraints already exist by the time the worker joins via
    /// `add_self`. Refuses a limit whose controller the unit's cgroup does
    /// not offer, rather than run the worker without it.
    #[allow(clippy::question_mark)]
    pub fn provision(
        tree: &CgroupTree,
        worker: &WorkerName,
        limits: &CgroupLimits,
    ) -> Result<Self> {
        let dir_name = worker.to_string();
        if WorkerName::parse(&dir_name).as_ref() != Some(worker) {
            return Err(anyhow!(
                "refusing a cgroup for malformed worker name {dir_name:?}"
            ));
        }
        let nodes = limits.nodes();
        let missing = tree.missing_controllers();
        if let Some(node) = nodes.iter().find(|n| missing.contains(&n.controller)) {
            return Err(anyhow!(
                "{worker} sets {}, but the {} controller is not delegated to riku",
                node.file,
                node.controller
            ));
        }

        let path = tree.apps_dir().join(dir_name);
        if let Err(e) = tree::create_cgroup(&path) {
            return Err(e);
        }
        for node in &nodes {
            if let Err(e) = write_node(&path, node.file, &node.value) {
                return Err(e);
            }
        }
        Ok(WorkerCgroup { path })
    }

    /// Move the *calling* process into this cgroup by writing its own PID
    /// to `cgroup.procs`. Must be called from within the worker's own
    /// `pre_exec` hook (after fork, before exec), joining from there,
    /// using the worker's real top-level PID, avoids the race that would
    /// exist if the parent tried to add the PID after `Command::spawn()`
    /// returns (by then the worker may already have started running
    /// unconstrained).
    pub fn add_self(&self) -> std::io::Result<()> {
        let pid = nix::unistd::getpid();
        std::fs::write(self.path.join("cgroup.procs"), pid.to_string())
    }

    /// Read `memory.events` and return the cumulative `oom_kill` counter,
    /// or `None` if the node can't be read (e.g. cgroup already removed).
    pub fn oom_kill_count(&self) -> Option<u64> {
        let content = fs::read_to_string(self.path.join("memory.events")).ok()?;
        content
            .lines()
            .find_map(|line| line.strip_prefix("oom_kill "))
            .and_then(|value| value.trim().parse().ok())
    }

    /// Kill every process left in the cgroup and remove it. This reaches
    /// processes that left the worker's process group (`setsid`, a double
    /// fork) but cannot leave the cgroup. Writes `cgroup.kill` where the
    /// kernel has it (Linux 5.14+), otherwise SIGKILLs each pid listed in
    /// `cgroup.procs`, then waits up to `KILL_SETTLE` for it to empty.
    pub fn kill_and_remove(&self) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }
        self.kill_members();
        let deadline = Instant::now() + KILL_SETTLE;
        while !self.member_pids().is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        self.cleanup()
    }

    fn kill_members(&self) {
        let kill_node = self.path.join("cgroup.kill");
        if kill_node.exists() && fs::write(&kill_node, "1").is_ok() {
            return;
        }
        for pid in self.member_pids() {
            let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
        }
    }

    fn member_pids(&self) -> Vec<i32> {
        match fs::read_to_string(self.path.join("cgroup.procs")) {
            Ok(procs) => procs
                .lines()
                .filter_map(|line| line.trim().parse().ok())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Remove the cgroup directory. Must be called after the worker has
    /// exited: the kernel refuses to rmdir a non-empty cgroup.
    pub fn cleanup(&self) -> Result<()> {
        match fs::remove_dir(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).context(format!("removing cgroup {}", self.path.display())),
        }
    }
}

fn write_node(dir: &Path, node: &str, value: &str) -> Result<()> {
    fs::write(dir.join(node), value)
        .with_context(|| format!("writing {} to {}/{}", value, dir.display(), node))
}
