//! What every successful deploy does last: the post deploy hooks, the
//! release record for `riku rollback`, and telling the pusher where the app
//! is live.

use std::collections::HashMap;
use std::path::Path;

use crate::config::own_cert::has_own_cert;
use crate::config::router::Router;
use crate::config::{settings, RikuPaths};
use crate::live_urls::{
    domain_tls, live_lines, path_lines, port_lines, url_host, Line, LiveUrl, TlsState,
};
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

/// The app's routing as the `active` router saw it, with its certificate
/// outcome. An error is shown to the pusher and drops only the URL lines.
#[allow(clippy::question_mark)]
fn routed(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    active: &Router,
) -> anyhow::Result<(AppRouting, Option<TlsState>)> {
    let routing = match routing_env(app, env, paths, active, Record::Peek) {
        Ok((_, routing)) => routing,
        Err(e) => return Err(e),
    };
    let tls = RoutingState::load(paths)
        .ok()
        .and_then(|state| state.entry(app).and_then(|entry| entry.tls));
    Ok((routing, tls))
}

/// Whether `app` gets a route at all: always for containers, else only a
/// Procfile app with a web facing copy (see `router::has_route`).
fn is_routed(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    container: bool,
) -> anyhow::Result<bool> {
    if container {
        return Ok(true);
    }
    crate::router::has_route(app, env, paths)
}

/// Where the app is live, as `live_urls` words it, and the notes around it.
/// Nothing for a Procfile app without a route.
#[allow(clippy::question_mark)]
fn live_lines_of(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    container: bool,
) -> anyhow::Result<Vec<Line>> {
    match is_routed(app, env, paths, container) {
        Ok(true) => {}
        Ok(false) => return Ok(Vec::new()),
        Err(e) => return Err(e),
    }
    let active = match Router::load(paths) {
        Ok(active) => active,
        Err(e) => return Err(e),
    };
    let (routing, tls) = match routed(app, env, paths, &active) {
        Ok(found) => found,
        Err(e) => return Err(e),
    };
    Ok(routing_lines(app, &routing, tls, &active, env, paths))
}

/// The lines for `app` routed as `routing` by `active`, with the port and
/// path URLs on the server's address.
fn routing_lines(
    app: &str,
    routing: &AppRouting,
    tls: Option<TlsState>,
    active: &Router,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Vec<Line> {
    let own_tls = domain_tls(
        active,
        crate::util::flag::https_only(env),
        has_own_cert(paths, app),
    );
    let mut lines = live_lines(app, routing, tls, own_tls);
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

/// The routing the last apply set up for `app`, from the routing state,
/// with its certificate outcome. `None` when the app was never routed.
#[allow(clippy::question_mark)]
fn applied_routing(
    app: &str,
    paths: &RikuPaths,
) -> anyhow::Result<Option<(AppRouting, Option<TlsState>)>> {
    let state = match RoutingState::load(paths) {
        Ok(state) => state,
        Err(e) => return Err(e),
    };
    let Some(entry) = state.entry(app) else {
        return Ok(None);
    };
    let routing = AppRouting {
        domains: entry.domains.clone(),
        fallback: entry.fallback,
        port: entry.port,
        mode: entry.mode,
        fallback_skipped: None,
        fallback_private_ip: None,
    };
    Ok(Some((routing, entry.tls)))
}

fn urls_of(lines: Vec<Line>) -> Vec<LiveUrl> {
    lines.into_iter().filter_map(|line| line.url).collect()
}

/// Where the dashboard says an app is live.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppUrls {
    /// The URLs of the route the last apply set up, which the router serves
    /// now.
    pub urls: Vec<LiveUrl>,
    /// The URLs the current settings give, when they differ from `urls`:
    /// the app answers there only after a redeploy. Empty otherwise.
    pub pending: Vec<LiveUrl>,
}

#[allow(clippy::question_mark)]
fn app_urls_of(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    container: bool,
) -> anyhow::Result<AppUrls> {
    match is_routed(app, env, paths, container) {
        Ok(true) => {}
        Ok(false) => return Ok(AppUrls::default()),
        Err(e) => return Err(e),
    }
    let active = match Router::load(paths) {
        Ok(active) => active,
        Err(e) => return Err(e),
    };
    let applied = match applied_routing(app, paths) {
        Ok(Some((routing, tls))) => urls_of(routing_lines(app, &routing, tls, &active, env, paths)),
        Ok(None) => return Ok(AppUrls::default()),
        Err(e) => return Err(e),
    };
    let fresh = match routed(app, env, paths, &active) {
        Ok((routing, tls)) => urls_of(routing_lines(app, &routing, tls, &active, env, paths)),
        Err(e) => return Err(e),
    };
    let pending = if fresh == applied { Vec::new() } else { fresh };
    Ok(AppUrls {
        urls: applied,
        pending,
    })
}

/// [`live_lines_of`] for the push output: a failure is shown to the pusher
/// and drops only the URL lines.
fn live_at(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    container: bool,
) -> Vec<Line> {
    match live_lines_of(app, env, paths, container) {
        Ok(lines) => lines,
        Err(e) => {
            display::warn(&format!("Cannot tell where the app is live: {e:#}"));
            Vec::new()
        }
    }
}

/// Where `app` is live for the dashboard, in the push output's URL form:
/// the route the last apply recorded, and the URLs a redeploy would give
/// when the settings changed since. `container` is whether the app runs as
/// containers; `env` is its merged config. Both are empty for an app that
/// was never routed or has no route, and when the routing cannot be read;
/// the push output shows that reason instead.
pub fn app_urls(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
    container: bool,
) -> AppUrls {
    match app_urls_of(app, env, paths, container) {
        Ok(urls) => urls,
        Err(e) => {
            tracing::debug!("no URLs for '{app}': {e:#}");
            AppUrls::default()
        }
    }
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
    let container = deployed.runtime == crate::containers::CONTAINER_RUNTIME;
    let urls = urls_after_notes(live_at(app, env, paths, container));
    let commit = git_ops::head_commit(&app_path);
    progress.succeeded(&Outcome {
        commit: commit.as_ref(),
        urls: &urls,
        workers: &deployed.workers,
    });
}

#[cfg(test)]
#[path = "deploy_finish_tests.rs"]
mod tests;
