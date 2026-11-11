//! Finishing a `riku restart` whose rewrite stopped part way, so the app is
//! left consistent instead of with some copies on a new env nobody records.

use std::collections::HashMap;

use riku_supervisor::config::{app_worker_configs, WorkerConfig};

use super::config_file::read_config;
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

/// `LIVE_ENV` describes `env` once any copy runs it, the supervisor reloads
/// the configs already written, and the returned error names the kinds and
/// copies on the new env and the kinds that may still run the old one.
/// `running` is what ran before the restart.
pub(super) fn finish_partial(
    app: &str,
    paths: &RikuPaths,
    env: &HashMap<String, String>,
    running: &[WorkerConfig],
    partial: PartialRewrite,
    reload_supervisor: impl Fn(&RikuPaths),
) -> anyhow::Error {
    let copies = match partial.left_old.first() {
        Some(failed_kind) => changed_copies(app, failed_kind, running, paths),
        None => Vec::new(),
    };
    let mut error = partial.cause.context(format!(
        "restart stopped part way; on the new env: {}; copies of the failed kind changed: {}; \
         possibly still on the old env: {}",
        listed(&partial.restarted),
        listed(&copies),
        listed(&partial.left_old)
    ));
    if !partial.restarted.is_empty() || !copies.is_empty() {
        if let Err(e) = env_setup::write_live_env(app, paths, env) {
            error = error.context(format!("LIVE_ENV not written: {e:#}"));
        }
    }
    reload_supervisor(paths);
    error
}
