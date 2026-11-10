//! A certificate for one fallback domain.
//!
//! Unlike [`crate::ensure_ssl_certificates`] this never makes a self-signed
//! certificate: a browser warning on a fallback URL is worse than plain
//! HTTP, so a failed issue leaves the app on HTTP. An existing pair is
//! reused, which keeps repeated pushes inside Let's Encrypt's limits;
//! acme.sh renews it on its own schedule.

use anyhow::{anyhow, Result};

use crate::acme::try_acme_certificates;
use crate::config::RikuPaths;
use crate::ssl::restrict_key;

/// Make sure `app` has a certificate for `domain`. Returns whether a pair is
/// now in `nginx_root`; `false` means plain HTTP only.
#[allow(clippy::question_mark)]
pub fn issue_single(app: &str, domain: &str, paths: &RikuPaths) -> Result<bool> {
    match crate::util::validate_app_name(app) {
        Ok(checked) if checked == app => {}
        _ => return Err(anyhow!("'{app}' is not a valid app name for a certificate")),
    }
    let key_path = paths.nginx_root.join(format!("{app}.key"));
    let chain_path = paths.nginx_root.join(format!("{app}.fullchain.crt"));
    if key_path.exists() && chain_path.exists() {
        return restrict_key(&key_path).map(|()| true);
    }
    let acme_sh = paths.acme_sh_root.join("acme.sh");
    if !acme_sh.exists() {
        return Ok(false);
    }
    match try_acme_certificates(app, &[domain.to_string()], paths, &acme_sh) {
        Ok(Some(_)) => restrict_key(&key_path).map(|()| true),
        Ok(None) => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "ssl_single_tests.rs"]
mod tests;
