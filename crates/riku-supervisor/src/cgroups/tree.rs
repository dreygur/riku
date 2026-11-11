//! The cgroup v2 subtree the supervisor owns, and how it takes it over.
//!
//! systemd hands the `riku` unit its own cgroup when the unit says
//! `Delegate=yes`, chowned to the unit's user. The supervisor finds that
//! cgroup in `/proc/self/cgroup` and lays it out as:
//!
//! - `<own>/supervisor`: the supervisor itself, and every worker without
//!   isolation, since children start in their parent's cgroup.
//! - `<own>/apps/<worker>`: one leaf per isolated worker.
//!
//! cgroup v2 lets a cgroup hand controllers to its children only while no
//! process sits in it, so the supervisor leaves `<own>` before it writes
//! `<own>/cgroup.subtree_control`.

// Project rule 16 forbids `?`, so every early return here is spelled out.
#![allow(clippy::question_mark)]

use anyhow::{anyhow, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use super::write_node;

/// Where the unified cgroup hierarchy is mounted.
pub const CGROUP_MOUNT: &str = "/sys/fs/cgroup";

/// Controllers a worker's limits use: `memory.max`, `cpu.max`, `pids.max`.
const WANTED_CONTROLLERS: [&str; 3] = ["memory", "cpu", "pids"];

const SUPERVISOR_LEAF: &str = "supervisor";
const APPS_DIR: &str = "apps";

/// The supervisor's own cgroup directory.
#[derive(Debug, Clone)]
pub struct CgroupTree {
    own: PathBuf,
    /// Wanted controllers `<own>` does not offer, known after `take_over`.
    missing: Vec<&'static str>,
}

impl CgroupTree {
    /// Finds the cgroup named on the `0::` line of `proc_self_cgroup` (the
    /// text of `/proc/self/cgroup`) under `mount`. Fails when there is no
    /// such line, which means cgroup v2 is not in use, or when the path is
    /// not a plain absolute one.
    pub fn locate(mount: &Path, proc_self_cgroup: &str) -> Result<CgroupTree> {
        let Some(line) = proc_self_cgroup
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
        else {
            return Err(anyhow!("no cgroup v2 entry in /proc/self/cgroup"));
        };
        let relative = match plain_relative(line) {
            Some(relative) => relative,
            None => return Err(anyhow!("unexpected cgroup path {line:?}")),
        };
        Ok(CgroupTree {
            own: mount.join(relative),
            missing: Vec::new(),
        })
    }

    /// Moves process `pid` into `<own>/supervisor`, then lets `<own>` and
    /// `<own>/apps` hand the wanted controllers down, and remembers the
    /// wanted controllers `<own>` does not offer. Fails when the cgroup is
    /// not delegated to this user.
    pub fn take_over(&mut self, pid: u32) -> Result<()> {
        let supervisor = self.own.join(SUPERVISOR_LEAF);
        if let Err(e) = create_cgroup(&supervisor) {
            return Err(e);
        }
        if let Err(e) = write_node(&supervisor, "cgroup.procs", &pid.to_string()) {
            return Err(e);
        }

        let offered = match fs::read_to_string(self.own.join("cgroup.controllers")) {
            Ok(text) => text,
            Err(e) => return Err(anyhow!("reading {}: {e}", self.own.display())),
        };
        let (enabled, missing): (Vec<&str>, Vec<&str>) = WANTED_CONTROLLERS
            .into_iter()
            .partition(|wanted| offered.split_whitespace().any(|c| c == *wanted));

        if let Err(e) = enable_controllers(&self.own, &enabled) {
            return Err(e);
        }
        let apps = self.apps_dir();
        if let Err(e) = create_cgroup(&apps) {
            return Err(e);
        }
        if let Err(e) = enable_controllers(&apps, &enabled) {
            return Err(e);
        }
        self.missing = missing;
        Ok(())
    }

    /// Controllers isolated workers cannot use, because the unit's cgroup
    /// does not offer them. Empty before `take_over`.
    pub fn missing_controllers(&self) -> &[&'static str] {
        &self.missing
    }

    /// The parent of every isolated worker's cgroup.
    pub fn apps_dir(&self) -> PathBuf {
        self.own.join(APPS_DIR)
    }
}

/// Locates and takes over the running supervisor's own cgroup.
pub fn take_over_own() -> Result<CgroupTree> {
    let text = match fs::read_to_string("/proc/self/cgroup") {
        Ok(text) => text,
        Err(e) => return Err(anyhow!("reading /proc/self/cgroup: {e}")),
    };
    let mut tree = match CgroupTree::locate(Path::new(CGROUP_MOUNT), &text) {
        Ok(tree) => tree,
        Err(e) => return Err(e),
    };
    match tree.take_over(std::process::id()) {
        Ok(()) => Ok(tree),
        Err(e) => Err(e),
    }
}

/// `path` without its leading `/`, or `None` when it is relative or has a
/// component other than a plain name.
fn plain_relative(path: &str) -> Option<&str> {
    let Some(relative) = path.strip_prefix('/') else {
        return None;
    };
    let plain = Path::new(relative)
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    plain.then_some(relative)
}

/// Creates the cgroup directory `path`, or reuses it when it exists.
pub(super) fn create_cgroup(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(anyhow!("creating cgroup {}: {e}", path.display())),
    }
}

fn enable_controllers(cgroup: &Path, controllers: &[&str]) -> Result<()> {
    if controllers.is_empty() {
        return Ok(());
    }
    let request = controllers
        .iter()
        .map(|controller| format!("+{controller}"))
        .collect::<Vec<_>>()
        .join(" ");
    write_node(cgroup, "cgroup.subtree_control", &request)
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
