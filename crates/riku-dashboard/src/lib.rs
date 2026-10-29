//! Embedded read-only dashboard (Track A, Phase 1).
//!
//! A small Axum server, compiled **into the riku binary**, that serves a
//! single embedded HTML page plus a read-only JSON API. No Node runtime, no
//! separate web stack: the single-binary identity is preserved.
//!
//! Once bound, it prints `riku-dashboard-bound <port>` on its own stdout
//! line, so a remote session (`riku dashboard -s <server>`) can learn the
//! port behind `--bind 127.0.0.1:0`.
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

use anyhow::{anyhow, Result};
use tokio::net::TcpListener;

use crate::cli::remote::dashboard::BOUND_PREFIX;
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
    let addr: SocketAddr = match bind.parse() {
        Ok(addr) => addr,
        Err(e) => return Err(anyhow!("invalid --bind address '{bind}': {e}")),
    };

    let token = token.or_else(|| std::env::var("RIKU_DASHBOARD_TOKEN").ok());

    // Refuse to expose the API beyond loopback without a token.
    if !is_loopback(&addr.ip()) && token.is_none() {
        return Err(anyhow!(
            "binding {addr} is non-loopback; set RIKU_DASHBOARD_TOKEN (or pass --token) first, \
             or bind 127.0.0.1 and tunnel over SSH"
        ));
    }

    let state = DashboardState {
        paths: paths.clone(),
        token,
    };

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => return Err(anyhow!("building tokio runtime: {e}")),
    };
    runtime.block_on(serve(addr, state))
}

/// Binds `addr` and returns the listener with the line that reports the port
/// it really got, which is how a remote session learns the port behind `:0`.
async fn bind_reporting(addr: SocketAddr) -> Result<(TcpListener, String)> {
    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => return Err(anyhow!("binding {addr}: {e}")),
    };
    let bound = match listener.local_addr() {
        Ok(bound) => bound,
        Err(e) => return Err(anyhow!("reading the address bound for {addr}: {e}")),
    };
    Ok((listener, format!("{BOUND_PREFIX}{}", bound.port())))
}

fn print_banner(bound: SocketAddr, has_token: bool) {
    display::info(&format!("Dashboard listening on http://{bound}"));
    if has_token {
        display::note("Token set: API requires it, and restart/stop/redeploy actions are enabled.");
    } else {
        display::note(
            "Read-only, loopback-only, no token (actions disabled). Tunnel with: ssh -L 8088:localhost:8088 <host>",
        );
    }
}

async fn serve(addr: SocketAddr, state: DashboardState) -> Result<()> {
    let (listener, report) = match bind_reporting(addr).await {
        Ok(bound) => bound,
        Err(e) => return Err(e),
    };
    println!("{report}");
    match listener.local_addr() {
        Ok(bound) => print_banner(bound, state.token.is_some()),
        Err(e) => return Err(anyhow!("reading the address bound for {addr}: {e}")),
    }
    let app = routes::router(state);
    match axum::serve(listener, app).await {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("serving dashboard: {e}")),
    }
}

fn is_loopback(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
