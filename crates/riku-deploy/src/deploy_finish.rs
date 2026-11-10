//! What every successful deploy does last: the post deploy hooks, the
//! release record for `riku rollback`, and telling the pusher where the app
//! is live.

use std::collections::HashMap;
use std::path::Path;

use crate::config::router::Router;
use crate::config::{settings, RikuPaths};
use crate::live_urls::{live_lines, path_lines, port_lines, TlsState};
use crate::router::app_routing::{AppRouting, RoutingMode};
use crate::router::routing_env::{routing_env, Record};
use crate::router::routing_state::RoutingState;
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

/// The app's routing as the router saw it, with its certificate outcome.
/// `None` when it cannot be read, which only drops the URL lines.
fn routed(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Option<(AppRouting, Option<TlsState>)> {
    let active = match Router::load(paths) {
        Ok(active) => active,
        Err(_) => return None,
    };
    let routing = match routing_env(app, env, paths, &active, Record::Peek) {
        Ok((_, routing)) => routing,
        Err(_) => return None,
    };
    let tls = RoutingState::load(paths)
        .ok()
        .and_then(|state| state.entry(app).and_then(|entry| entry.tls));
    Some((routing, tls))
}

fn announce(app: &str, env: &HashMap<String, String>, paths: &RikuPaths) {
    echo(&format!("-----> {} deployed!", app), "green");
    let Some((routing, tls)) = routed(app, env, paths) else {
        return;
    };
    let mut lines = live_lines(app, &routing, tls, crate::util::flag::https_only(env));
    let server = settings::load(paths).ok().map(|loaded| loaded.routing);
    let server_ip = server.as_ref().and_then(|routing| routing.server_ip);
    if let Some(port) = routing.port {
        lines.extend(port_lines(port, server_ip));
    }
    if let (RoutingMode::Path, Some(server)) = (routing.mode, &server) {
        lines.extend(path_lines(app, server.shared_port, server_ip));
    }
    for line in lines {
        echo(&format!("       {}", line.text), line.colour);
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
    announce(app, env, paths);
}
