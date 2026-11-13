//! Finishing a `riku restart` whose rewrite stopped part way, so the app is
//! left consistent instead of with some copies on a new env nobody records.

use std::collections::HashMap;

use riku_supervisor::config::{app_worker_configs, WorkerConfig};

use super::config_file::read_config;
use super::LiveEnv;
use crate::config::RikuPaths;
use crate::env_setup;
use crate::workers::PartialRewrite;

/// The enabled copies of `kind` that differ from the `running` configs the
/// restart started from: a new copy name (a rolled web copy takes a free
/// slot) or a rewritten env. These run the new env.
fn changed_copies(
    app: &str,
    kind: &str,
    running: &[WorkerConfig],
    paths: &RikuPaths,
) -> Vec<String> {
    let before: HashMap<String, &WorkerConfig> = running
        .iter()
        .map(|config| (config.worker.name().to_string(), config))
        .collect();
    let mut changed: Vec<String> = app_worker_configs(&paths.workers_enabled, app)
        .iter()
        .filter_map(|path| read_config(path, paths).ok())
        .filter(|config| config.worker.kind == kind)
        .filter(
            |config| match before.get(&config.worker.name().to_string()) {
                Some(old) => old.env != config.env,
                None => true,
            },
        )
        .map(|config| config.worker.name().to_string())
        .collect();
    changed.sort();
    changed
}

fn listed(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

/// The restart that stopped part way.
pub(super) struct Restart<'a> {
    pub app: &'a str,
    pub paths: &'a RikuPaths,
    /// The env the restart was rewriting the configs to.
    pub env: &'a HashMap<String, String>,
    /// What ran before the restart.
    pub running: &'a [WorkerConfig],
    pub live_env: LiveEnv,
}

/// `LIVE_ENV` describes the new env once any copy runs it (only when the
/// restart covered every enabled kind), the supervisor reloads the configs
/// already written, and the returned error names the kinds and copies on
/// the new env and the kinds that may still run the old one.
pub(super) fn finish_partial(
    restart: &Restart<'_>,
    partial: PartialRewrite,
    reload_supervisor: impl Fn(&RikuPaths),
) -> anyhow::Error {
    let (app, paths) = (restart.app, restart.paths);
    let copies = match partial.left_old.first() {
        Some(failed_kind) => changed_copies(app, failed_kind, restart.running, paths),
        None => Vec::new(),
    };
    let mut error = partial.cause.context(format!(
        "restart stopped part way; on the new env: {}; copies of the failed kind changed: {}; \
         possibly still on the old env: {}",
        listed(&partial.restarted),
        listed(&copies),
        listed(&partial.left_old)
    ));
    let any_new = !partial.restarted.is_empty() || !copies.is_empty();
    if any_new && restart.live_env == LiveEnv::Write {
        if let Err(e) = env_setup::write_live_env(app, paths, restart.env) {
            error = error.context(format!("LIVE_ENV not written: {e:#}"));
        }
    }
    reload_supervisor(paths);
    error
}
