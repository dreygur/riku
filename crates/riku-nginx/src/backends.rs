//! Where nginx sends an app's traffic when nothing else says otherwise.
//!
//! This module only reads the app's ENV and the path layout. The values it
//! returns are checked for nginx directive injection at render time, in
//! [`crate::template`].

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::util::backend::Backend;

const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1";

/// The ENV key and value that name the app's upstream port: `RIKU_UPSTREAM_PORT`,
/// then `PORT`, then 8080 under the key `PORT`.
fn upstream_port_entry(env: &HashMap<String, String>) -> (&'static str, &str) {
    if let Some(value) = env.get("RIKU_UPSTREAM_PORT") {
        return ("RIKU_UPSTREAM_PORT", value);
    }
    match env.get("PORT") {
        Some(value) => ("PORT", value),
        None => ("PORT", "8080"),
    }
}

/// The app's upstream port as written in its ENV.
pub(crate) fn upstream_port(env: &HashMap<String, String>) -> String {
    upstream_port_entry(env).1.to_string()
}

/// The name of the app's nginx `upstream` block: `riku_` and the lowercase hex
/// of the app name's bytes.
///
/// nginx compares upstream names case-insensitively across every included
/// site, while riku app names keep their case, so `MyApp` and `myapp` must not
/// map to names that differ only in case. Hex is case-free and only uses
/// characters nginx accepts in a name.
pub(crate) fn upstream_name(app: &str) -> String {
    let hex: String = app.bytes().map(|byte| format!("{byte:02x}")).collect();
    format!("riku_{hex}")
}

/// The app's unix socket: `RIKU_NGINX_SOCKET`, else `<socket_root>/<app>.sock`.
pub(crate) fn socket_path(
    app: &str,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
) -> String {
    match env.get("RIKU_NGINX_SOCKET") {
        Some(socket) => socket.clone(),
        None => paths
            .socket_root
            .join(format!("{app}.sock"))
            .to_string_lossy()
            .to_string(),
    }
}

/// The address the app listens on: `RIKU_BIND_ADDRESS`, default `127.0.0.1`.
/// A value that is not an IP literal (`localhost` included) is an error.
pub fn bind_address(env: &HashMap<String, String>) -> Result<IpAddr> {
    let bind = env
        .get("RIKU_BIND_ADDRESS")
        .map_or(DEFAULT_BIND_ADDRESS, String::as_str);
    match bind.trim().parse() {
        Ok(host) => Ok(host),
        Err(_) => Err(anyhow!("RIKU_BIND_ADDRESS is not an IP address: '{bind}'")),
    }
}

/// The app's upstream port: `RIKU_UPSTREAM_PORT`, then `PORT`, default 8080.
/// A value that is present but not a port number is an error naming its key.
pub fn upstream_port_number(env: &HashMap<String, String>) -> Result<u16> {
    let (key, value) = upstream_port_entry(env);
    match value.trim().parse() {
        Ok(port) => Ok(port),
        Err(_) => Err(anyhow!("{key} is not a port number: '{value}'")),
    }
}

#[allow(clippy::question_mark)]
fn tcp_backend(env: &HashMap<String, String>) -> Result<Backend> {
    let host = match bind_address(env) {
        Ok(host) => host,
        Err(e) => return Err(e),
    };
    let port = match upstream_port_number(env) {
        Ok(port) => port,
        Err(e) => return Err(e),
    };
    Ok(Backend::Tcp { host, port })
}

/// The single backend an app gets when the caller has no better list.
///
/// A port mapped app (`RIKU_NGINX_PORTMAP`) is reached at `RIKU_BIND_ADDRESS`
/// (default `127.0.0.1`) on its upstream port; every other app on its unix
/// socket. A bind address or port that is present but malformed is an error.
pub fn default_backends(
    app: &str,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
) -> Result<Vec<Backend>> {
    if env.contains_key("RIKU_NGINX_PORTMAP") {
        return match tcp_backend(env) {
            Ok(backend) => Ok(vec![backend]),
            Err(e) => Err(e),
        };
    }
    Ok(vec![Backend::Unix(PathBuf::from(socket_path(
        app, env, paths,
    )))])
}

#[cfg(test)]
#[path = "backends_tests.rs"]
mod tests;
