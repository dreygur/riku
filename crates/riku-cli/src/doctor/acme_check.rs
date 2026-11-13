//! `riku doctor` check for acme.sh, which riku issues certificates with
//! when nginx is the router. Caddy issues its own, so it needs none.

use crate::config::router::Router;
use crate::config::RikuPaths;

use super::Check;

/// Whether acme.sh is installed where deploys look for it, under nginx
/// only. Without it a fallback domain and an `https_only` app stay on
/// plain HTTP.
pub fn acme_sh(paths: &RikuPaths) -> Option<Check> {
    if !matches!(Router::load(paths), Ok(Router::Nginx)) {
        return None;
    }
    let acme_sh = paths.acme_sh_root.join("acme.sh");
    if acme_sh.is_file() {
        return Some(Check::ok(
            "acme.sh",
            format!("found at {}", acme_sh.display()),
        ));
    }
    Some(Check::warn(
        "acme.sh",
        format!(
            "not found at {}: apps get no Let's Encrypt certificate; install it as the deploy user: cd ~ && curl -fsSL https://get.acme.sh | sh",
            acme_sh.display()
        ),
    ))
}

#[cfg(test)]
#[path = "acme_check_tests.rs"]
mod tests;
