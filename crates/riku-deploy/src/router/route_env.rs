//! The parts of an app's route that come from ENV keys and cert files: where
//! the router listens, the proxy cache and the TLS pair. Values are parsed
//! into the route's types here; whether they are safe is the gate's call in
//! `riku_route::check_route`, not this module's.

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_container::types::AppName;
use riku_route::{Cache, Listen, SiteKind, Tls};

use crate::config::{self, RikuPaths};
use crate::util::flag::is_on;

const IPV4_KEY: &str = "RIKU_NGINX_IPV4_ADDRESS";
const IPV6_KEY: &str = "RIKU_NGINX_IPV6_ADDRESS";
const DISABLE_IPV6_KEY: &str = "RIKU_DISABLE_IPV6";
const EXTERNAL_PORT_KEY: &str = "RIKU_NGINX_EXTERNAL_PORT";
const CACHE_PREFIXES_KEY: &str = "RIKU_NGINX_CACHE_PREFIXES";
const WSGI_KEY: &str = "RIKU_NGINX_WSGI";
const DEFAULT_HTTP_PORT: u16 = 80;

/// Read in this order by [`cache`]: time, size, expiry, redirects, any, control.
const CACHE_NUMBER_KEYS: [(&str, u32); 6] = [
    ("RIKU_NGINX_CACHE_TIME", config::NGINX_CACHE_TIME_DEFAULT),
    ("RIKU_NGINX_CACHE_SIZE", config::NGINX_CACHE_SIZE_DEFAULT),
    (
        "RIKU_NGINX_CACHE_EXPIRY",
        config::NGINX_CACHE_EXPIRY_DEFAULT,
    ),
    (
        "RIKU_NGINX_CACHE_REDIRECTS",
        config::NGINX_CACHE_REDIRECTS_DEFAULT,
    ),
    ("RIKU_NGINX_CACHE_ANY", config::NGINX_CACHE_ANY_DEFAULT),
    (
        "RIKU_NGINX_CACHE_CONTROL",
        config::NGINX_CACHE_CONTROL_DEFAULT,
    ),
];

fn parsed<T: std::str::FromStr>(env: &HashMap<String, String>, key: &str, default: T) -> Result<T> {
    match env.get(key) {
        None => Ok(default),
        Some(value) => match value.trim().parse() {
            Ok(parsed) => Ok(parsed),
            Err(_) => Err(anyhow!("{key} is not valid: '{value}'")),
        },
    }
}

fn ipv6_address(env: &HashMap<String, String>) -> Result<Option<Ipv6Addr>> {
    if env.get(DISABLE_IPV6_KEY).is_some_and(|v| is_on(v)) {
        return Ok(None);
    }
    let Some(value) = env.get(IPV6_KEY) else {
        return Ok(Some(Ipv6Addr::UNSPECIFIED));
    };
    if value.trim().is_empty() {
        return Ok(None);
    }
    let bare = value.trim().trim_start_matches('[').trim_end_matches(']');
    match bare.parse() {
        Ok(address) => Ok(Some(address)),
        Err(_) => Err(anyhow!("{IPV6_KEY} is not an IPv6 address: '{value}'")),
    }
}

/// Where nginx listens. Only a port mapped or uWSGI site uses
/// `RIKU_NGINX_EXTERNAL_PORT`, and never under `https_only`, as in the
/// templates.
pub(super) fn listen(
    env: &HashMap<String, String>,
    kind: SiteKind,
    https_only: bool,
) -> Result<Listen> {
    let bind: Ipv4Addr = match parsed(env, IPV4_KEY, Ipv4Addr::UNSPECIFIED) {
        Ok(bind) => bind,
        Err(e) => return Err(anyhow!("cannot read the listen address: {e}")),
    };
    let ipv6 = match ipv6_address(env) {
        Ok(ipv6) => ipv6,
        Err(e) => return Err(anyhow!("cannot read the IPv6 listen address: {e}")),
    };
    let maps_port = matches!(kind, SiteKind::Wsgi | SiteKind::Portmap);
    let port = if maps_port && !https_only {
        match parsed(env, EXTERNAL_PORT_KEY, DEFAULT_HTTP_PORT) {
            Ok(port) => port,
            Err(e) => return Err(anyhow!("cannot read the listen port: {e}")),
        }
    } else {
        DEFAULT_HTTP_PORT
    };
    Ok(Listen { bind, port, ipv6 })
}

fn cache_numbers(env: &HashMap<String, String>) -> Result<[u32; 6]> {
    let mut numbers = [0; 6];
    for (slot, (key, default)) in numbers.iter_mut().zip(CACHE_NUMBER_KEYS) {
        *slot = match parsed(env, key, default) {
            Ok(number) => number,
            Err(e) => return Err(anyhow!("cannot read the cache settings: {e}")),
        };
    }
    Ok(numbers)
}

/// The proxy cache, present when `RIKU_NGINX_CACHE_PREFIXES` names a prefix.
/// `RIKU_NGINX_CACHE_PATH` is refused earlier, so no path comes from here.
pub(super) fn cache(env: &HashMap<String, String>) -> Result<Option<Cache>> {
    let listed = env.get(CACHE_PREFIXES_KEY).map_or("", String::as_str);
    let prefixes: Vec<String> = listed
        .split(',')
        .map(|prefix| prefix.trim().to_string())
        .filter(|prefix| !prefix.is_empty())
        .collect();
    if prefixes.is_empty() || env.contains_key(WSGI_KEY) {
        return Ok(None);
    }
    match cache_numbers(env) {
        Ok([time_s, size_gb, expiry_s, redirects_s, any_s, control_s]) => Ok(Some(Cache {
            prefixes,
            time_s,
            size_gb,
            expiry_s,
            redirects_s,
            any_s,
            control_s,
        })),
        Err(e) => Err(e),
    }
}

fn read_pem(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow!("cannot read {}: {e}", path.display())),
    }
}

/// The app's certificate chain and key from `nginx_root`, where acme.sh or
/// the self signed fallback left them. `None` when neither file exists; one
/// without the other is an error. The full chain is preferred over the leaf.
pub(super) fn tls(app: &AppName, paths: &RikuPaths) -> Result<Option<Tls>> {
    let stem = paths.nginx_root.join(app.as_str());
    let with_suffix = |suffix: &str| {
        let mut name = stem.clone().into_os_string();
        name.push(suffix);
        std::path::PathBuf::from(name)
    };
    let files = read_pem(&with_suffix(".fullchain.crt"))
        .and_then(|chain| match chain {
            Some(chain) => Ok(Some(chain)),
            None => read_pem(&with_suffix(".crt")),
        })
        .and_then(|chain| read_pem(&with_suffix(".key")).map(|key| (chain, key)));
    match files {
        Ok((Some(cert_pem), Some(key_pem))) => Ok(Some(Tls { cert_pem, key_pem })),
        Ok((None, None)) => Ok(None),
        Ok(_) => Err(anyhow!(
            "'{app}' has only one of its certificate and key in {}",
            paths.nginx_root.display()
        )),
        Err(e) => Err(e),
    }
}
