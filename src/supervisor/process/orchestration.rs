//! Lifecycle orchestration: versioned generations, the active health-probe
//! loop, and the rollback circuit breaker.
//!
//! A deploy of an already-running process slot spawns a *new* generation
//! under a temporary key instead of tearing the old one down immediately.
//! A background thread polls the new generation's health endpoint for a
//! fixed window; the main supervisor loop (via `reconcile_generations`,
//! called once per tick from `Supervisor::run`) drains probe outcomes and
//! either promotes the new generation into the canonical process slot or
//! trips the circuit breaker and rolls back, leaving the previous stable
//! generation serving traffic the entire time.

use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::supervisor::config::WorkerConfig;

use super::generation::{AppGeneration, GenerationStatus, ProbeOutcome};
use super::ProcessManager;

/// Total time a new generation is given to start returning `200 OK`.
const PROBE_WINDOW: Duration = Duration::from_secs(5);
/// Delay between probe attempts within the window.
const PROBE_INTERVAL: Duration = Duration::from_millis(250);
/// Ordinal offset applied to a canary's temporary process_id so it never
/// collides with the stable generation's key (mirrors `hot_reload.rs`).
const CANARY_ORDINAL_OFFSET: u32 = u32::MAX / 2;
/// How many `Failed` generations to keep per process_id as an audit trail
/// before the oldest ones are pruned.
const MAX_FAILED_HISTORY: usize = 5;

impl ProcessManager {
    /// Deploy a new generation of `process_id` from `new_config`, probing it
    /// in the background before it ever takes over traffic.
    ///
    /// `process_id` must already be running (this is the canary path for
    /// redeploys, not the initial spawn). The existing stable process is
    /// left untouched and continues serving traffic until the new
    /// generation either passes its probe window (promoted) or fails it
    /// (rolled back).
    pub fn deploy_generation(&mut self, process_id: &str, new_config: WorkerConfig) -> Result<()> {
        let port: u16 = new_config
            .env
            .get("PORT")
            .and_then(|p| p.parse().ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "cannot probe generation for {}: no PORT env var configured",
                    process_id
                )
            })?;

        let health_path = new_config
            .options
            .health_check
            .as_ref()
            .map(|h| h.url.clone())
            .unwrap_or_else(|| "/health".to_string());

        let canonical_ordinal = new_config.worker.ordinal;
        let next_version = self
            .generations
            .get(process_id)
            .and_then(|gens| gens.iter().map(|g| g.version).max())
            .unwrap_or(0)
            + 1;

        let mut temp_config = new_config;
        temp_config.worker.ordinal =
            canonical_ordinal.wrapping_add(CANARY_ORDINAL_OFFSET + next_version);
        let temp_key = format!(
            "{}-{}-{}",
            temp_config.worker.app, temp_config.worker.kind, temp_config.worker.ordinal
        );

        tracing::info!(
            "Deploying generation v{} for {} (temp key {}, probing {}:{}{})",
            next_version,
            process_id,
            temp_key,
            "127.0.0.1",
            port,
            health_path
        );

        self.spawn_process(&temp_config)?;
        let pid = self
            .processes
            .get(&temp_key)
            .map(|p| p.pid_as_u32())
            .unwrap_or(0);

        let gens = self.generations.entry(process_id.to_string()).or_default();
        gens.push(AppGeneration {
            version: next_version,
            pids: vec![pid],
            status: GenerationStatus::Probing,
            temp_key: temp_key.clone(),
            canonical_ordinal,
        });

        // Prune old `Failed` audit entries beyond the retention window so the
        // ring doesn't grow unbounded across many redeploy attempts.
        let mut failed_seen = 0usize;
        for gen in gens.iter_mut().rev() {
            if gen.status == GenerationStatus::Failed {
                failed_seen += 1;
            }
        }
        if failed_seen > MAX_FAILED_HISTORY {
            let mut to_drop = failed_seen - MAX_FAILED_HISTORY;
            gens.retain(|g| {
                if g.status == GenerationStatus::Failed && to_drop > 0 {
                    to_drop -= 1;
                    false
                } else {
                    true
                }
            });
        }

        let results = Arc::clone(&self.probe_results);
        let probe_process_id = process_id.to_string();

        thread::spawn(move || {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new());
            let url = format!("http://127.0.0.1:{port}{health_path}");
            let deadline = Instant::now() + PROBE_WINDOW;

            let mut healthy = false;
            let mut reason = "probe window expired without a 200 OK".to_string();

            while Instant::now() < deadline {
                match client.get(&url).send() {
                    Ok(resp) if resp.status().is_success() => {
                        healthy = true;
                        break;
                    }
                    Ok(resp) => reason = format!("probe returned HTTP {}", resp.status()),
                    Err(e) => reason = format!("probe request failed: {e}"),
                }
                thread::sleep(PROBE_INTERVAL);
            }

            let outcome = ProbeOutcome {
                process_id: probe_process_id,
                version: next_version,
                healthy,
                reason,
            };
            if let Ok(mut guard) = results.lock() {
                guard.push(outcome);
            }
        });

        Ok(())
    }

    /// Drain probe outcomes and detect generations whose process crashed
    /// before the probe thread finished. Must be called once per
    /// supervisor tick (alongside `check_processes`).
    pub fn reconcile_generations(&mut self) -> Result<()> {
        // Detect a canary that crashed outright before its probe completed.
        let mut crashed: Vec<(String, u32, String)> = Vec::new();
        for (process_id, gens) in self.generations.iter() {
            for gen in gens
                .iter()
                .filter(|g| g.status == GenerationStatus::Probing)
            {
                let still_running = self
                    .processes
                    .get_mut(&gen.temp_key)
                    .map(|p| p.is_running())
                    .unwrap_or(false);
                if !still_running {
                    crashed.push((
                        process_id.clone(),
                        gen.version,
                        "process exited before the probe window completed".to_string(),
                    ));
                }
            }
        }
        for (process_id, version, reason) in crashed {
            self.fail_generation(&process_id, version, &reason);
        }

        // Drain whatever the background probe threads have reported.
        let outcomes: Vec<ProbeOutcome> = {
            let mut guard = self
                .probe_results
                .lock()
                .map_err(|_| anyhow::anyhow!("probe_results mutex poisoned"))?;
            std::mem::take(&mut *guard)
        };

        for outcome in outcomes {
            if outcome.healthy {
                self.promote_generation(&outcome.process_id, outcome.version)?;
            } else {
                self.fail_generation(&outcome.process_id, outcome.version, &outcome.reason);
            }
        }

        Ok(())
    }

    /// Promote a probed generation into the canonical process slot,
    /// terminating the previous stable generation it replaces.
    fn promote_generation(&mut self, process_id: &str, version: u32) -> Result<()> {
        let Some(gens) = self.generations.get_mut(process_id) else {
            return Ok(());
        };
        let Some(idx) = gens
            .iter()
            .position(|g| g.version == version && g.status == GenerationStatus::Probing)
        else {
            return Ok(());
        };
        let temp_key = gens[idx].temp_key.clone();
        let canonical_ordinal = gens[idx].canonical_ordinal;

        if let Some(mut old) = self.processes.remove(process_id) {
            let grace = old.config.options.grace_period;
            old.terminate()?;
            let deadline = Duration::from_secs(grace);
            let poll = Duration::from_millis(100);
            let mut elapsed = Duration::ZERO;
            while old.is_running() && elapsed < deadline {
                thread::sleep(poll);
                elapsed += poll;
            }
            if old.is_running() {
                old.kill()?;
            }
        }
        self.stats.remove_process(process_id);

        if let Some(mut new_process) = self.processes.remove(&temp_key) {
            new_process.config.worker.ordinal = canonical_ordinal;
            let app = new_process.config.worker.app.clone();
            let kind = new_process.config.worker.kind.clone();
            self.stats.remove_process(&temp_key);
            self.stats
                .register_process(process_id.to_string(), app, kind, canonical_ordinal);
            self.stats
                .mark_running(process_id, new_process.pid_as_u32());
            self.processes.insert(process_id.to_string(), new_process);
        }

        let gens = self.generations.get_mut(process_id).unwrap();
        gens.retain(|g| g.version == version);
        gens[0].status = GenerationStatus::Stable;

        tracing::info!(
            "Promoted {} to generation v{} (stable)",
            process_id,
            version
        );
        self.deployment_events.push(format!(
            "[DEPLOYMENT_PROMOTED] {process_id} v{version} is now stable"
        ));
        Ok(())
    }

    /// Trip the rollback circuit breaker: kill the failed generation's
    /// process and drop it from the ring. The previous stable generation
    /// (still under `process_id` in `self.processes`) is never touched, so
    /// traffic routing never moves off it.
    fn fail_generation(&mut self, process_id: &str, version: u32, reason: &str) {
        let Some(gens) = self.generations.get_mut(process_id) else {
            return;
        };
        let Some(idx) = gens.iter().position(|g| g.version == version) else {
            return;
        };
        let temp_key = gens[idx].temp_key.clone();
        // Keep the entry as a `Failed` audit record rather than dropping it
        // outright — only the OS process underneath it is torn down.
        gens[idx].status = GenerationStatus::Failed;
        gens[idx].pids.clear();

        if let Some(mut failed) = self.processes.remove(&temp_key) {
            let _ = failed.terminate();
            thread::sleep(Duration::from_millis(100));
            if failed.is_running() {
                let _ = failed.kill();
            }
        }
        self.stats.remove_process(&temp_key);

        tracing::error!(
            "Generation v{} for {} failed: {}. Rolling back — traffic stays on the previous stable generation.",
            version,
            process_id,
            reason
        );
        self.deployment_events.push(format!(
            "[DEPLOYMENT_FAILED - ROLLING_BACK] {process_id} v{version}: {reason}"
        ));
    }

    /// Take and clear all pending deployment notifications. Called once per
    /// tick by the daemon loop so they can be pushed onto the same
    /// broadcast channel as the metrics SSE stream, non-blockingly.
    pub fn drain_deployment_events(&mut self) -> Vec<String> {
        std::mem::take(&mut self.deployment_events)
    }
}

/// Shared, lock-protected outcome queue written by probe threads and
/// drained by `reconcile_generations` on the main supervisor thread.
pub(super) type ProbeResults = Arc<Mutex<Vec<ProbeOutcome>>>;

pub(super) fn new_probe_results() -> ProbeResults {
    Arc::new(Mutex::new(Vec::new()))
}
