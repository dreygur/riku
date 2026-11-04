//! Process stopping and termination logic for the ProcessManager.
//!
//! A worker runs as the leader of its own process group, and every signal
//! goes to the whole group. Group members can outlive the leader, so a stop
//! is only done once the group is empty. An isolated worker also lives in a
//! cgroup, which is killed and removed after the group, so processes that
//! left the group (`setsid`, a double-forked daemon) die with it. A worker
//! without isolation has no cgroup: whatever left its group survives the
//! stop.
//!
//! Every stop goes through `SpawnedProcess::stop`: the normal stop, the
//! promotion and rollback of a generation, and the cleanup in `Drop`.

use anyhow::{anyhow, Result};
use std::thread;
use std::time::{Duration, Instant};

use super::{ProcessManager, SpawnedProcess};

/// How long a SIGKILLed group gets to disappear before the stop gives up.
const KILL_SETTLE: Duration = Duration::from_secs(2);
const STOP_POLL: Duration = Duration::from_millis(100);

/// Polls `still_waiting` until it says false or `limit` passes.
fn wait_while(limit: Duration, mut still_waiting: impl FnMut() -> bool) {
    let deadline = Instant::now() + limit;
    while still_waiting() && Instant::now() < deadline {
        thread::sleep(STOP_POLL);
    }
}

// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#[allow(clippy::question_mark)]
impl SpawnedProcess {
    /// Stops the process group, then kills and removes the cgroup. The
    /// cgroup step runs even when the group step fails. Marks the process
    /// stopped so `Drop` does not stop it a second time.
    ///
    /// The process keeps its pid marker until it is dropped, after this
    /// returns, so deploy waits also cover the leftover group members.
    pub(crate) fn stop(&mut self, process_id: &str) -> Result<()> {
        tracing::info!("Stopping process: {} (PID: {})", process_id, self.pid);
        self.stopped = true;
        let group = self.stop_group(process_id);
        self.kill_cgroup(process_id);
        if group.is_ok() {
            tracing::info!("Process {} stopped", process_id);
        }
        group
    }

    /// SIGTERM, wait up to the configured `grace_period`, then SIGKILL if
    /// the leader is still alive, and the same again for group members left
    /// once the leader is reaped.
    fn stop_group(&mut self, process_id: &str) -> Result<()> {
        if let Err(e) = self.terminate() {
            return Err(e);
        }
        let grace = Duration::from_secs(self.config.options.grace_period);
        wait_while(grace, || self.is_running());

        if self.is_running() {
            tracing::warn!(
                "Process {} didn't respond to SIGTERM within {:?}, sending SIGKILL",
                process_id,
                grace
            );
            if let Err(e) = self.kill() {
                return Err(e);
            }
        }
        if let Err(e) = self.child.wait() {
            return Err(anyhow!("cannot reap {process_id}: {e}"));
        }
        self.stop_group_leftovers(process_id, grace)
    }

    /// Members of the group already had their SIGTERM with the leader, so
    /// they get the same grace before the group is SIGKILLed.
    fn stop_group_leftovers(&mut self, process_id: &str, grace: Duration) -> Result<()> {
        if !self.group_alive() {
            return Ok(());
        }
        tracing::info!("{process_id}: group members outlived the leader, waiting {grace:?}");
        wait_while(grace, || self.group_alive());
        if !self.group_alive() {
            return Ok(());
        }

        tracing::warn!("{process_id}: group members ignored SIGTERM, sending SIGKILL");
        if let Err(e) = self.kill() {
            return Err(e);
        }
        wait_while(KILL_SETTLE, || self.group_alive());
        if self.group_alive() {
            tracing::warn!("{process_id}: group still has members {KILL_SETTLE:?} after SIGKILL");
        }
        Ok(())
    }

    fn kill_cgroup(&self, process_id: &str) {
        let Some(cgroup) = &self.cgroup else {
            return;
        };
        if let Err(e) = cgroup.kill_and_remove() {
            tracing::warn!("{process_id}: cgroup left behind: {e:#}");
        }
    }
}

#[allow(clippy::question_mark)]
impl ProcessManager {
    /// Stop a specific process by its ID.
    pub(crate) fn stop_process_by_id(&mut self, process_id: &str) -> Result<()> {
        if let Some(mut process) = self.processes.remove(process_id) {
            self.stats.mark_stopped(process_id);
            if let Err(e) = process.stop(process_id) {
                return Err(e);
            }
        }
        Ok(())
    }

    /// Stop all processes for a specific app.
    pub fn stop_app_processes(&mut self, app_name: &str) -> Result<()> {
        let processes_to_remove: Vec<String> = self
            .processes
            .iter()
            .filter(|(_, process)| process.config.worker.app == app_name)
            .map(|(id, _)| id.clone())
            .collect();

        for process_id in processes_to_remove {
            self.stop_process_by_id(&process_id)?;
        }
        Ok(())
    }

    /// Stop all managed processes, respecting each process's configured grace_period.
    ///
    /// Each process is stopped on its own thread so total shutdown time is
    /// bounded by the *slowest* process's grace_period, not the sum of all
    /// of them: stopping N processes sequentially on the single-threaded
    /// supervisor shutdown path would otherwise serialize N grace-period
    /// waits.
    pub fn stop_all_processes(&mut self) -> Result<()> {
        let process_ids: Vec<String> = self.processes.keys().cloned().collect();

        let mut to_stop: Vec<(String, SpawnedProcess)> = Vec::new();
        for process_id in process_ids {
            if let Some(process) = self.processes.remove(&process_id) {
                self.stats.mark_stopped(&process_id);
                to_stop.push((process_id, process));
            }
        }

        thread::scope(|scope| {
            for (process_id, process) in &mut to_stop {
                scope.spawn(move || {
                    if let Err(e) = process.stop(process_id) {
                        tracing::error!("Error stopping process {}: {:?}", process_id, e);
                    }
                });
            }
        });

        Ok(())
    }
}

#[cfg(test)]
#[path = "stop_tests.rs"]
mod tests;
