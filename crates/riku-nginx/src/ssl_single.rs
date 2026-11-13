//! A certificate for one fallback domain.
//!
//! Unlike [`crate::ensure_ssl_certificates`] this never makes a self-signed
//! certificate: a browser warning on a fallback URL is worse than plain
//! HTTP, so a failed issue leaves the app on HTTP. An existing pair is
//! reused, which keeps repeated pushes inside Let's Encrypt's limits;
//! acme.sh renews it on its own schedule.

use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::acme::try_acme_certificates;
use crate::config::RikuPaths;
use crate::ssl::restrict_key;

/// How [`issue_single`] ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SingleCert {
    /// A pair for the domain is in `nginx_root`.
    Ready,
    /// acme.sh ran and got no certificate: plain HTTP for now.
    Failed,
    /// acme.sh is not installed at this path, so nothing was asked of Let's
    /// Encrypt.
    NoAcme(PathBuf),
}

/// Make sure `app` has a certificate for `domain`. Anything but
/// [`SingleCert::Ready`] means plain HTTP only.
#[allow(clippy::question_mark)]
pub fn issue_single(app: &str, domain: &str, paths: &RikuPaths) -> Result<SingleCert> {
    match crate::util::validate_app_name(app) {
        Ok(checked) if checked == app => {}
        _ => return Err(anyhow!("'{app}' is not a valid app name for a certificate")),
    }
    let ready = |ok: bool| match ok {
        true => SingleCert::Ready,
        false => SingleCert::Failed,
    };
    if crate::config::own_cert::has_own_cert(paths, app) {
        return crate::ssl::own_pair_ready(app, paths).map(ready);
    }
    let key_path = paths.nginx_root.join(format!("{app}.key"));
    let chain_path = paths.nginx_root.join(format!("{app}.fullchain.crt"));
    if key_path.exists() && chain_path.exists() {
        return restrict_key(&key_path).map(|()| SingleCert::Ready);
    }
    let acme_sh = paths.acme_sh_root.join("acme.sh");
    if !acme_sh.exists() {
        return Ok(SingleCert::NoAcme(acme_sh));
    }
    match try_acme_certificates(app, &[domain.to_string()], paths, &acme_sh) {
        Ok(Some(_)) => restrict_key(&key_path).map(|()| SingleCert::Ready),
        Ok(None) => Ok(SingleCert::Failed),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "ssl_single_tests.rs"]
mod tests;
