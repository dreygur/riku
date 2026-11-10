//! The shared path mode site for Caddy: one `http://:<port>` site with a
//! `handle_path /<app>/*` block per app, which strips the prefix, and the
//! prefix sent on as `X-Forwarded-Prefix`.
//!
//! Security: app names have passed `AppName` in the router helper; they are
//! checked again here so nothing but letters, digits, `-`, `_` and `.` ever
//! reaches the Caddyfile. Backends are TCP only, as for a single app site.

use anyhow::{anyhow, Result};
use riku_util::backend::Backend;

fn safe_name(app: &str) -> bool {
    !app.is_empty()
        && app
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

#[allow(clippy::question_mark)]
fn app_block(app: &str, backends: &[Backend]) -> Result<String> {
    if !safe_name(app) {
        return Err(anyhow!("app name {app:?} is not safe in a Caddyfile"));
    }
    if backends.is_empty() {
        return Err(anyhow!("'{app}' has no backend to proxy to"));
    }
    if backends.iter().any(|b| matches!(b, Backend::Unix(_))) {
        return Err(anyhow!(
            "Caddy proxies TCP only, not unix sockets ('{app}')"
        ));
    }
    let upstreams: Vec<String> = backends.iter().map(Backend::to_string).collect();
    Ok(format!(
        "\tredir /{app} /{app}/\n\thandle_path /{app}/* {{\n\t\treverse_proxy {} {{\n\t\t\theader_up X-Forwarded-Prefix /{app}\n\t\t}}\n\t}}\n",
        upstreams.join(" ")
    ))
}

/// The site file text for every path mode app on `port`.
#[allow(clippy::question_mark)]
pub fn render_paths(port: u16, apps: &[(String, Vec<Backend>)]) -> Result<String> {
    let blocks: Result<Vec<String>> = apps
        .iter()
        .map(|(app, backends)| app_block(app, backends))
        .collect();
    let blocks = match blocks {
        Ok(blocks) => blocks,
        Err(e) => return Err(e),
    };
    Ok(format!(
        "# riku path apps\nhttp://:{port} {{\n{}}}\n",
        blocks.concat()
    ))
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
