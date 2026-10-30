//! Where nginx sends an app's traffic when nothing else says otherwise.
//!
//! This module only reads the app's ENV and the path layout. The values it
//! returns are checked for nginx directive injection at render time, in
//! [`crate::template`].

use std::collections::HashMap;
use std::path::PathBuf;

use crate::util::backend::Backend;

const DEFAULT_UPSTREAM_PORT: &str = "8080";

/// The app's upstream port as written in its ENV: `RIKU_UPSTREAM_PORT`, then
/// `PORT`, then 8080.
pub(crate) fn upstream_port(env: &HashMap<String, String>) -> String {
    env.get("RIKU_UPSTREAM_PORT")
        .or_else(|| env.get("PORT"))
        .cloned()
        .unwrap_or_else(|| DEFAULT_UPSTREAM_PORT.to_string())
}

/// The app's unix socket: `RIKU_NGINX_SOCKET`, else `<nginx_root>/<app>.sock`.
pub(crate) fn socket_path(
    app: &str,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
) -> String {
    match env.get("RIKU_NGINX_SOCKET") {
        Some(socket) => socket.clone(),
        None => paths
            .nginx_root
            .join(format!("{app}.sock"))
            .to_string_lossy()
            .to_string(),
    }
}

/// The single backend an app gets when the caller has no better list.
///
/// A port mapped app (`RIKU_NGINX_PORTMAP`) is reached on its upstream port;
/// every other app on its unix socket. An upstream port that is not a valid
/// port number falls back to 8080.
pub fn default_backends(
    app: &str,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
) -> Vec<Backend> {
    if env.contains_key("RIKU_NGINX_PORTMAP") {
        let port = upstream_port(env).trim().parse().unwrap_or(8080);
        return vec![Backend::Tcp(port)];
    }
    vec![Backend::Unix(PathBuf::from(socket_path(app, env, paths)))]
}

#[cfg(test)]
#[path = "backends_tests.rs"]
mod tests;
