//! The one backend an app has when it runs no web facing copy: a worker only
//! app, an app without a Procfile, a static site. Its router config is still
//! written, pointing where it always pointed.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};

use anyhow::{anyhow, Result};

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::backend::Backend;

/// nginx gets its own pick from the app's ENV. Caddy reaches the app over
/// loopback whatever `RIKU_BIND_ADDRESS` says. A router plugin gets the
/// address the app listens on, since it may run elsewhere and must not be
/// told a made up address.
pub(super) fn fallback_backends(
    router: &Router,
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<Vec<Backend>> {
    match router {
        Router::Nginx => match crate::nginx::default_backends(app, env, paths) {
            Ok(backends) => Ok(backends),
            Err(e) => Err(anyhow!("cannot route '{app}' through nginx: {e}")),
        },
        Router::Caddy => tcp_backend(env, IpAddr::V4(Ipv4Addr::LOCALHOST)),
        Router::Plugin(name) => {
            let host = match crate::nginx::bind_address(env) {
                Ok(host) => host,
                Err(e) => return Err(anyhow!("cannot route '{app}' through '{name}': {e}")),
            };
            tcp_backend(env, host)
        }
    }
}

fn tcp_backend(env: &HashMap<String, String>, host: IpAddr) -> Result<Vec<Backend>> {
    match crate::nginx::upstream_port_number(env) {
        Ok(port) => Ok(vec![Backend::Tcp { host, port }]),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "fallback_tests.rs"]
mod tests;
