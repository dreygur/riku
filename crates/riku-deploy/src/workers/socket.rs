//! Check for an app that names its uWSGI socket with `RIKU_NGINX_SOCKET`.
//!
//! One path cannot be shared by several copies, so such an app runs one uWSGI
//! process. The check runs before any worker config changes.

use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;

use super::read_scaling_count;

const WSGI_KINDS: [&str; 4] = ["wsgi", "jwsgi", "rwsgi", "php"];

/// Refuse a custom `RIKU_NGINX_SOCKET` when any uWSGI kind is scaled
/// past one process. A TCP web app ignores the key.
#[allow(clippy::question_mark)]
pub(super) fn check_custom_socket(
    app: &str,
    env: &HashMap<String, String>,
    paths: &RikuPaths,
) -> Result<()> {
    if !env.contains_key("RIKU_NGINX_SOCKET") {
        return Ok(());
    }
    for kind in WSGI_KINDS {
        let count = match read_scaling_count(paths, app, kind) {
            Ok(count) => count,
            Err(e) => return Err(e),
        };
        if count > 1 {
            return Err(anyhow!(
                "{kind} is scaled to {count} but RIKU_NGINX_SOCKET names one socket; remove RIKU_NGINX_SOCKET to run several copies"
            ));
        }
    }
    Ok(())
}
