//! Refusing a socket path the app names with `RIKU_NGINX_SOCKET`.
//!
//! Security: the router helper picks every socket path the route points at,
//! and the route build refuses the key (see `router::refused_key`). A web
//! facing copy must not listen on a path the app picked either: it would
//! bind wherever the app says, outside the socket directory, and no route
//! would reach it.

use std::collections::HashMap;

use anyhow::{anyhow, Result};

/// Fail with the route build's reason when `env` sets a key it refuses.
pub(super) fn refuse_app_socket(env: &HashMap<String, String>) -> Result<()> {
    match crate::router::refused_key(env) {
        Some(reason) => Err(anyhow!(reason)),
        None => Ok(()),
    }
}
