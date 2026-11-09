//! What every successful deploy does last: the post deploy hooks, the
//! release record for `riku rollback`, and telling the pusher where the app
//! is live.

use std::collections::HashMap;
use std::path::Path;

use crate::config::RikuPaths;
use crate::util::{deploy_logger::DeployLogger, echo};
use crate::{git_ops, hooks, releases};

fn record_release(app: &str, app_path: &Path, paths: &RikuPaths) {
    let Some(sha) = git_ops::head_sha(app_path) else {
        return;
    };
    if let Err(e) = releases::ReleaseLog::new(paths).record(app, &sha) {
        tracing::warn!("could not record release for '{}': {}", app, e);
    }
}

fn announce(app: &str, env: &HashMap<String, String>) {
    echo(&format!("-----> {} deployed!", app), "green");
    match env.get("RIKU_DOMAINS").filter(|d| !d.is_empty()) {
        Some(domain) => {
            let scheme = if crate::util::flag::https_only(env) {
                "https"
            } else {
                "http"
            };
            echo(&format!("       Live at {}://{}", scheme, domain), "green");
        }
        None => echo(
            &format!(
                "       Add a domain: riku settings set {} routing.domains=example.com",
                app
            ),
            "",
        ),
    }
}

/// Run the post deploy hooks (their failures are warnings only), record the
/// deployed revision and print where the app is live.
pub(crate) fn finish_deploy(
    app: &str,
    paths: &RikuPaths,
    runtime: &str,
    env: &HashMap<String, String>,
    dlog: &mut DeployLogger,
) {
    let app_path = paths.app_root.join(app);
    dlog.log_raw("Running post-deploy hooks");
    let _ = hooks::run_post_deploy(app, &app_path, paths, Some(runtime), env);
    dlog.log(&format!("Deploy of '{}' complete", app));
    record_release(app, &app_path, paths);
    announce(app, env);
}
