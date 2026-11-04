//! The probes a health wait polls: an HTTP GET, a TCP connect and a unix
//! socket connect, plus the polling loop around them.
//!
//! # Security model
//!
//! HTTP and TCP probes only connect to `127.0.0.1`. The HTTP client follows
//! no redirect and ignores system proxy settings, so a copy cannot point the
//! probe at another service. A socket probe connects to the path riku wrote
//! into the copy's own worker config.

use std::fs;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use reqwest::redirect::Policy;

const PROBE_INTERVAL: Duration = Duration::from_millis(500);
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const GROUP_READ_WRITE: u32 = 0o060;

pub(super) fn probe_client() -> Result<Client> {
    match riku_http::blocking_client_builder()
        .timeout(PROBE_TIMEOUT)
        .redirect(Policy::none())
        .no_proxy()
        .build()
    {
        Ok(client) => Ok(client),
        Err(e) => Err(anyhow!("cannot build the health probe client: {e}")),
    }
}

pub(super) fn probe_url(client: &Client, port: u16, path: &str) -> Result<()> {
    let slash = if path.starts_with('/') { "" } else { "/" };
    let url = format!("http://127.0.0.1:{port}{slash}{path}");
    match client.get(url).send() {
        Ok(response) if response.status().is_success() => Ok(()),
        Ok(response) => Err(anyhow!("answered {}", response.status())),
        Err(e) => Err(anyhow!("request failed: {e}")),
    }
}

pub(super) fn probe_port(port: u16) -> Result<()> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    match TcpStream::connect_timeout(&addr, PROBE_TIMEOUT) {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("connect failed: {e}")),
    }
}

/// One socket poll: `Ok(false)` while nothing listens, `Err` once a
/// listener is up on a socket its group cannot read and write.
///
/// The probe connects as the deploy user, who owns the socket, so a connect
/// alone cannot see what nginx sees. nginx connects through the deploy
/// group (root init adds it), and without group access every request is
/// a 502 while the deploy looks healthy.
pub(super) fn probe_socket(path: &Path) -> Result<bool> {
    if UnixStream::connect(path).is_err() {
        return Ok(false);
    }
    let mode = match fs::metadata(path) {
        Ok(meta) => meta.permissions().mode() & 0o777,
        Err(e) => return Err(anyhow!("cannot inspect {}: {e}", path.display())),
    };
    if mode & GROUP_READ_WRITE != GROUP_READ_WRITE {
        return Err(anyhow!(
            "{} is mode {mode:o}, so nginx cannot connect to it; \
             the socket needs group read and write (uWSGI chmod-socket=660)",
            path.display()
        ));
    }
    Ok(true)
}

/// Calls `attempt` every `PROBE_INTERVAL` until it returns `Ok(true)`, an
/// error, or `limit` passes.
pub(super) fn poll_until<F>(limit: Duration, mut attempt: F) -> Result<()>
where
    F: FnMut() -> Result<bool>,
{
    let deadline = Instant::now() + limit;
    loop {
        match attempt() {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => return Err(e),
        }
        if Instant::now() >= deadline {
            return Err(anyhow!("not healthy after {limit:?}"));
        }
        sleep(PROBE_INTERVAL);
    }
}

/// Retries `probe` until it succeeds or `limit` passes; the error names the
/// last failure.
pub(super) fn poll_probe(limit: Duration, probe: impl Fn() -> Result<()>) -> Result<()> {
    let mut last = String::from("no answer");
    let outcome = poll_until(limit, || match probe() {
        Ok(()) => Ok(true),
        Err(e) => {
            last = e.to_string();
            Ok(false)
        }
    });
    match outcome {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("{e} (last probe: {last})")),
    }
}
