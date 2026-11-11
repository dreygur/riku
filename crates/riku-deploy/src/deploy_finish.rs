//! What every successful deploy does last: the post deploy hooks, the
//! release record for `riku rollback`, and telling the pusher where the app
//! is live.

use std::collections::HashMap;
use std::path::Path;

use crate::config::router::Router;
use crate::config::{settings, RikuPaths};
use crate::live_urls::{live_lines, path_lines, port_lines, url_host, Line, TlsState};
use crate::procfile_deploy::Deployed;
use crate::progress::{DeployProgress, Outcome};
use crate::router::app_routing::{AppRouting, RoutingMode};
use crate::router::routing_env::{routing_env, Record};
use crate::router::routing_state::RoutingState;
use crate::util::display;
use crate::util::outbound_ip::detect_outbound_ip;
use crate::{git_ops, hooks, releases};

/// The prefix of a live URL line; the summary lists the URL alone.
const LIVE_AT: &str = "Live at ";

fn record_release(app: &str, app_path: &Path, paths: &RikuPaths) {
    let Some(sha) = git_ops::head_sha(app_path) else {
        return;
    };
    if let Err(e) = releases::ReleaseLog::new(paths).record(app, &sha) {
        tracing::warn!("could not record release for '{}': {}", app, e);
    }
}

/// The app's routing as the router saw it, with its certificate outcome.
/// An error is shown to the pusher and drops only the URL lines.
#[allow(clippy::question_mark)]
fn routed(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> anyhow::Result<(AppRouting, Option<TlsState>)> {
    let active = match Router::load(paths) {
        Ok(active) => active,
        Err(e) => return Err(e),
    };
    let routing = match routing_env(app, env, paths, &active, Record::Peek) {
        Ok((_, routing)) => routing,
        Err(e) => return Err(e),
    };
    let tls = RoutingState::load(paths)
        .ok()
        .and_then(|state| state.entry(app).and_then(|entry| entry.tls));
    Ok((routing, tls))
}

/// Where the app is live, as `live_urls` words it.
fn live_at(app: &str, env: &HashMap<String, String>, paths: &RikuPaths) -> Vec<Line> {
    let (routing, tls) = match routed(app, env, paths) {
        Ok(found) => found,
        Err(e) => {
            display::warn(&format!("Cannot tell where the app is live: {e:#}"));
            return Vec::new();
        }
    };
    let mut lines = live_lines(app, &routing, tls, crate::util::flag::https_only(env));
    let server = settings::load(paths).ok().map(|loaded| loaded.routing);
    let configured = server.as_ref().and_then(|routing| routing.server_ip);
    let needs_host = routing.port.is_some() || routing.mode == RoutingMode::Path;
    let server_ip = match (configured, needs_host) {
        (None, true) => url_host(None, detect_outbound_ip()),
        _ => configured,
    };
    if let Some(port) = routing.port {
        lines.extend(port_lines(port, server_ip));
    }
    if let (RoutingMode::Path, Some(server)) = (routing.mode, &server) {
        lines.extend(path_lines(app, server.shared_port, server_ip));
    }
    lines
}

/// Splits `lines` into the URLs for the summary, printing every other line
/// on the rail before the outro.
fn urls_after_notes(lines: Vec<Line>) -> Vec<String> {
    let mut urls = Vec::new();
    for line in lines {
        match line.text.strip_prefix(LIVE_AT) {
            Some(url) => urls.push(url.to_string()),
            None if line.colour == "yellow" => display::warn(&line.text),
            None => display::detail(&line.text),
        }
    }
    urls
}

/// Run the post deploy hooks (their failures are warnings only), record the
/// deployed revision and print the outro with where the app is live.
pub(crate) fn finish_deploy(
    app: &str,
    paths: &RikuPaths,
    deployed: &Deployed,
    env: &HashMap<String, String>,
    progress: &mut DeployProgress,
) {
    let app_path = paths.app_root.join(app);
    progress.log.log_raw("Running post-deploy hooks");
    let _ = hooks::run_post_deploy(app, &app_path, paths, Some(&deployed.runtime), env);
    record_release(app, &app_path, paths);
    let urls = urls_after_notes(live_at(app, env, paths));
    let commit = git_ops::head_commit(&app_path);
    progress.succeeded(&Outcome {
        commit: commit.as_ref(),
        urls: &urls,
        workers: &deployed.workers,
    });
}
