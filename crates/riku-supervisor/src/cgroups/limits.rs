//! The limits a worker's cgroup gets, and how each one is written into its
//! limit file.

use anyhow::Result;
use std::path::Path;

use super::write_node;

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

/// One limit file: the controller that owns it, its file, and what to write.
pub(super) struct LimitNode {
    pub(super) controller: &'static str,
    pub(super) file: &'static str,
    value: NodeValue,
}

/// What a limit file gets.
enum NodeValue {
    /// The limit the config sets.
    Set(String),
    /// A value that goes with a set limit, written only where the kernel
    /// offers the file (`memory.swap.max` needs swap accounting).
    SetWherePresent(String),
    /// No limit is set: the unlimited value, written only over a file that
    /// already exists, so a reused cgroup drops a limit an older config set.
    Unlimited(String),
}

impl LimitNode {
    fn new(
        controller: &'static str,
        file: &'static str,
        limit: Option<String>,
        unlimited: String,
    ) -> LimitNode {
        let value = match limit {
            Some(limit) => NodeValue::Set(limit),
            None => NodeValue::Unlimited(unlimited),
        };
        LimitNode {
            controller,
            file,
            value,
        }
    }

    pub(super) fn is_set(&self) -> bool {
        matches!(self.value, NodeValue::Set(_))
    }
}

impl CgroupLimits {
    /// Every limit file, with the limit or the unlimited value. A memory
    /// limit also sets `memory.swap.max` to 0: with swap the worker would
    /// page out past `memory.max` instead of being OOM killed.
    pub(super) fn nodes(&self) -> Vec<LimitNode> {
        let period = self.cpu_period_us;
        let swap = match self.memory_max_bytes {
            Some(_) => NodeValue::SetWherePresent("0".to_string()),
            None => NodeValue::Unlimited("max".to_string()),
        };
        vec![
            LimitNode::new(
                "memory",
                "memory.max",
                self.memory_max_bytes.map(|bytes| bytes.to_string()),
                "max".to_string(),
            ),
            LimitNode {
                controller: "memory",
                file: "memory.swap.max",
                value: swap,
            },
            LimitNode::new(
                "cpu",
                "cpu.max",
                self.cpu_quota_us.map(|quota| format!("{quota} {period}")),
                format!("max {period}"),
            ),
            LimitNode::new(
                "pids",
                "pids.max",
                self.pids_max.map(|pids| pids.to_string()),
                "max".to_string(),
            ),
        ]
    }
}

/// Writes `node` into the cgroup at `path`. An unset limit is written as
/// unlimited only when its controller is delegated and the file exists:
/// the kernel shows a controller's files only once it is enabled.
pub(super) fn apply_node(path: &Path, node: &LimitNode, missing: &[&str]) -> Result<()> {
    match &node.value {
        NodeValue::Set(value) => write_node(path, node.file, value),
        NodeValue::SetWherePresent(value) => {
            if !path.join(node.file).exists() {
                return Ok(());
            }
            write_node(path, node.file, value)
        }
        NodeValue::Unlimited(value) => {
            if missing.contains(&node.controller) || !path.join(node.file).exists() {
                return Ok(());
            }
            write_node(path, node.file, value)
        }
    }
}
