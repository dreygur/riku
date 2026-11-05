//! The parts of an app's route that come from ENV keys and cert files: where
//! the router listens and the TLS pair. Values are parsed into the route's
//! types here; whether they are safe is the gate's call in
//! `riku_route::check_route`, not this module's.

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_container::types::AppName;
use riku_route::{Listen, SiteKind, Tls};

use crate::config::RikuPaths;

const EXTERNAL_PORT_KEY: &str = "RIKU_NGINX_EXTERNAL_PORT";
const DEFAULT_HTTP_PORT: u16 = 80;

/// The plain HTTP port. Only a port mapped or uWSGI site uses
/// `RIKU_NGINX_EXTERNAL_PORT`, and never under `https_only`, as in the
/// templates.
pub(super) fn listen(
    env: &HashMap<String, String>,
    kind: SiteKind,
    https_only: bool,
) -> Result<Listen> {
    let maps_port = matches!(kind, SiteKind::Wsgi | SiteKind::Portmap);
    let (Some(value), true) = (env.get(EXTERNAL_PORT_KEY), maps_port && !https_only) else {
        return Ok(Listen {
            port: DEFAULT_HTTP_PORT,
        });
    };
    match value.trim().parse() {
        Ok(port) => Ok(Listen { port }),
        Err(_) => Err(anyhow!(
            "cannot read the listen port: {EXTERNAL_PORT_KEY} is not valid: '{value}'"
        )),
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
