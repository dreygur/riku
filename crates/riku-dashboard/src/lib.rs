//! Embedded read-only dashboard (Track A, Phase 1).
//!
//! A small Axum server, compiled **into the riku binary**, that serves a
//! single embedded HTML page plus a read-only JSON API. No Node runtime, no
//! separate web stack — the single-binary identity is preserved.
//!
//! Security posture (read-only first, per the roadmap):
//! - Binds `127.0.0.1` by default; read-only status data is low-sensitivity and
//!   already exposed there by the health/metrics endpoints.
//! - Binding a non-loopback address requires a token (`RIKU_DASHBOARD_TOKEN`),
//!   enforced on the API as `Authorization: Bearer` or `?token=`.
//! - A `Host` allowlist rejects non-loopback Host headers to blunt DNS-rebinding
//!   from a browser. Mutating actions (later) will add CSRF on top.

// Dependency crates aliased as their former module names.
pub(crate) use riku_cli as cli;
pub(crate) use riku_config as config;
pub(crate) use riku_deploy as deploy;
pub(crate) use riku_plugins as plugins;
pub(crate) use riku_supervisor as supervisor;
pub(crate) use riku_util as util;

mod addons;
mod appcfg;
mod installed;
mod logs;
mod market;
mod mutations;
mod routes;
mod system;

use std::net::{IpAddr, SocketAddr};

use anyhow::{Context, Result};

use crate::config::RikuPaths;
use crate::util::display;

/// Shared handler state.
#[derive(Clone)]
pub(crate) struct DashboardState {
    pub paths: RikuPaths,
    /// When set, the API requires this token.
    pub token: Option<String>,
}

/// Run the dashboard, blocking until the process is stopped. `bind` is a
/// `host:port` string; `token` (or `RIKU_DASHBOARD_TOKEN`) gates the API.
pub fn run(paths: &RikuPaths, bind: &str, token: Option<String>) -> Result<()> {
    let addr: SocketAddr = bind
        .parse()
        .with_context(|| format!("invalid --bind address '{bind}'"))?;

    let token = token.or_else(|| std::env::var("RIKU_DASHBOARD_TOKEN").ok());

    // Refuse to expose the API beyond loopback without a token.
    if !is_loopback(&addr.ip()) && token.is_none() {
        anyhow::bail!(
            "binding {addr} is non-loopback; set RIKU_DASHBOARD_TOKEN (or pass --token) first, \
             or bind 127.0.0.1 and tunnel over SSH"
        );
    }

    let state = DashboardState {
        paths: paths.clone(),
        token,
    };

    display::info(&format!("Dashboard listening on http://{addr}"));
    if state.token.is_some() {
        display::note("Token set: API requires it, and restart/stop/redeploy actions are enabled.");
    } else {
        display::note(
            "Read-only, loopback-only, no token (actions disabled). Tunnel with: ssh -L 8088:localhost:8088 <host>",
        );
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;
    runtime.block_on(serve(addr, state))
}

async fn serve(addr: SocketAddr, state: DashboardState) -> Result<()> {
    let app = routes::router(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    axum::serve(listener, app)
        .await
        .context("serving dashboard")?;
    Ok(())
}

fn is_loopback(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}
