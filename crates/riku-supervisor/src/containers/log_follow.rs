//! Container log streaming into copy log files.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Stdio};

use anyhow::{anyhow, Result};
use riku_container::engine::{ContainerId, ContainerInfo, Engine};

/// Starting position for following container logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tail {
    FromNow,
    All,
}

impl Tail {
    fn as_tail_lines(self) -> Option<u32> {
        match self {
            Tail::FromNow => Some(0),
            Tail::All => None,
        }
    }
}

/// Manages log-following child processes per container copy.
pub(crate) struct LogFollowers {
    log_root: PathBuf,
    children: HashMap<ContainerId, Child>,
}

impl LogFollowers {
    pub(crate) fn new(log_root: PathBuf) -> Self {
        Self {
            log_root,
            children: HashMap::new(),
        }
    }

    /// Follow logs of `info` into `{log_root}/{app}/{service}.{copy}.log`.
    pub(crate) fn follow(
        &mut self,
        engine: &Engine,
        info: &ContainerInfo,
        tail: Tail,
    ) -> Result<()> {
        if self.children.contains_key(&info.id) {
            return Ok(());
        }

        let app = match &info.app {
            Some(a) => a,
            None => return Ok(()),
        };
        let service = match &info.service {
            Some(s) => s,
            None => return Ok(()),
        };
        let copy = match &info.copy {
            Some(c) => c,
            None => return Ok(()),
        };

        let app_dir = self.log_root.join(app);
        match fs::create_dir_all(&app_dir) {
            Ok(()) => {}
            Err(e) => return Err(anyhow!("cannot create log directory: {e}")),
        }

        let log_path = app_dir.join(format!("{service}.{copy}.log"));
        let file = match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(f) => f,
            Err(e) => return Err(anyhow!("cannot open log file: {e}")),
        };

        let stderr_file = match file.try_clone() {
            Ok(f) => f,
            Err(e) => return Err(anyhow!("cannot clone log file handle: {e}")),
        };

        let mut cmd = engine.logs_command(&info.id, tail.as_tail_lines());
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::from(file));
        cmd.stderr(Stdio::from(stderr_file));

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return Err(anyhow!("cannot spawn log follower: {e}")),
        };

        self.children.insert(info.id.clone(), child);
        Ok(())
    }

    /// Check which followers exited, remove them, and return their ids.
    pub(crate) fn reap(&mut self) -> Vec<ContainerId> {
        let mut exited = Vec::new();
        self.children.retain(|id, child| match child.try_wait() {
            Ok(Some(_)) => {
                exited.push(id.clone());
                false
            }
            Ok(None) => true,
            Err(_) => {
                exited.push(id.clone());
                false
            }
        });
        exited
    }

    /// Terminate followers for any containers not present in `live`.
    pub(crate) fn retain(&mut self, live: &HashSet<ContainerId>) {
        let dead_ids: Vec<ContainerId> = self
            .children
            .keys()
            .filter(|id| !live.contains(id))
            .cloned()
            .collect();
        for id in dead_ids {
            if let Some(mut child) = self.children.remove(&id) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

impl Drop for LogFollowers {
    fn drop(&mut self) {
        for mut child in self.children.drain().map(|(_, child)| child) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
#[path = "log_follow_tests.rs"]
mod tests;
