//! Finishing a `riku restart` whose rewrite stopped part way, so the app is
//! left consistent instead of with some kinds on a new env nobody records.

use std::collections::HashMap;

use crate::config::RikuPaths;
use crate::env_setup;
use crate::workers::PartialRewrite;

/// `LIVE_ENV` describes `env` once any kind fully runs it, the supervisor
/// reloads the configs already written, and the returned error names which
/// kinds run the new env and which may still run the old one.
pub(super) fn finish_partial(
    app: &str,
    paths: &RikuPaths,
    env: &HashMap<String, String>,
    partial: PartialRewrite,
    reload_supervisor: impl Fn(&RikuPaths),
) -> anyhow::Error {
    let restarted = if partial.restarted.is_empty() {
        "none".to_string()
    } else {
        partial.restarted.join(", ")
    };
    let mut error = partial.cause.context(format!(
        "restart stopped part way; on the new env: {restarted}; possibly still on the old env: {}",
        partial.left_old.join(", ")
    ));
    if !partial.restarted.is_empty() {
        if let Err(e) = env_setup::write_live_env(app, paths, env) {
            error = error.context(format!("LIVE_ENV not written: {e:#}"));
        }
    }
    reload_supervisor(paths);
    error
}
