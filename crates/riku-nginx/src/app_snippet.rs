//! The app's own nginx snippet, named by `RIKU_NGINX_INCLUDE_FILE` and
//! copied into the app's generated `server` block.
//!
//! Security model: the file comes from the app's repo, and every riku site
//! is loaded into the one server-wide nginx. Three checks stand between the
//! repo and that nginx, and any failure refuses the deploy with the reason:
//!
//! 1. The file must resolve inside the app directory, symlinks followed
//!    first, and must exist.
//! 2. Its text must fit the narrow grammar in [`super::snippet_grammar`],
//!    so riku and nginx split it into the same statements.
//! 3. Every statement must start with a directive in [`ALLOWED_DIRECTIVES`]
//!    (headers, expiry, body size, timeouts, proxy buffering, gzip), spelled
//!    exactly.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Result};

use super::snippet_grammar::statements;

/// The env key naming the snippet, relative to the app directory.
pub(super) const SNIPPET_KEY: &str = "RIKU_NGINX_INCLUDE_FILE";

/// The only directives a snippet may use. Left out on purpose: `root`,
/// `alias`, `proxy_pass`, `return`, `rewrite` and `set` (they reroute or
/// serve other files), `include`, every `*_log` and `ssl_*` (they read or
/// write files as nginx's root master or swap TLS), every `limit_*` (zones
/// live at http level), and `proxy_set_header` (it could spoof forwarding
/// headers).
const ALLOWED_DIRECTIVES: &[&str] = &[
    "add_header",
    "expires",
    "client_max_body_size",
    "client_body_timeout",
    "keepalive_timeout",
    "proxy_connect_timeout",
    "proxy_read_timeout",
    "proxy_send_timeout",
    "proxy_buffering",
    "proxy_buffer_size",
    "proxy_buffers",
    "proxy_busy_buffers_size",
    "gzip",
    "gzip_types",
    "gzip_min_length",
    "gzip_comp_level",
    "gzip_vary",
];

/// Refuse snippet text that does not fit the grammar or uses a directive
/// outside the allowlist.
fn check_snippet(content: &str, path: &Path) -> Result<()> {
    let parsed = match statements(content) {
        Ok(parsed) => parsed,
        Err(reason) => return Err(anyhow!("{} {reason}", path.display())),
    };
    let refused = parsed
        .iter()
        .filter_map(|statement| statement.split_whitespace().next())
        .find(|name| !ALLOWED_DIRECTIVES.contains(name));
    match refused {
        Some(name) => Err(anyhow!(
            "{} uses `{name}`, which a {SNIPPET_KEY} snippet may not; allowed: {}",
            path.display(),
            ALLOWED_DIRECTIVES.join(", ")
        )),
        None => Ok(()),
    }
}

/// The snippet's checked content, or an empty string when the app sets no
/// `RIKU_NGINX_INCLUDE_FILE`. A missing file, one resolving outside the app
/// directory, or refused content fails the deploy.
pub(super) fn read_app_snippet(
    env: &std::collections::HashMap<String, String>,
    app_path: &Path,
) -> Result<String> {
    let Some(name) = env.get(SNIPPET_KEY) else {
        return Ok(String::new());
    };
    let path = app_path.join(name);
    let app_dir = match fs::canonicalize(app_path) {
        Ok(dir) => dir,
        Err(e) => return Err(anyhow!("cannot resolve {}: {e}", app_path.display())),
    };
    let resolved = match fs::canonicalize(&path) {
        Ok(resolved) => resolved,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(anyhow!(
                "{SNIPPET_KEY} names {}, which does not exist",
                path.display()
            ))
        }
        Err(e) => return Err(anyhow!("cannot resolve {}: {e}", path.display())),
    };
    if !resolved.starts_with(&app_dir) {
        return Err(anyhow!(
            "{SNIPPET_KEY} names {}, which resolves outside the app directory to {}",
            path.display(),
            resolved.display()
        ));
    }
    let content = match fs::read_to_string(&resolved) {
        Ok(content) => content,
        Err(e) => return Err(anyhow!("cannot read {}: {e}", path.display())),
    };
    match check_snippet(&content, &path) {
        Ok(()) => Ok(content),
        Err(e) => Err(e),
    }
}

/// Read the app's checked snippet as the seed value, run it through the
/// `nginx.include_content` filter chain (`PLUGIN_PROTOCOL.md` §7.3: lets an
/// installed plugin augment the generated config without replacing it), and
/// insert the result. The checks apply to the app's file, not to what an
/// installed plugin adds. Always inserts the key, even as an empty string,
/// so it's never simply absent from the template context.
#[allow(clippy::question_mark)]
pub(super) fn insert_include_file(
    context: &mut tera::Context,
    env: &std::collections::HashMap<String, String>,
    app_path: &Path,
    paths: &crate::config::RikuPaths,
) -> Result<()> {
    let content = match read_app_snippet(env, app_path) {
        Ok(content) => content,
        Err(e) => return Err(e),
    };

    let filtered = riku_plugins::FilterBus::new(paths)
        .apply("nginx.include_content", serde_json::json!(content));
    match filtered.as_str() {
        Some(s) => context.insert("RIKU_NGINX_INCLUDE_CONTENT", s),
        // A filter returned something that isn't a string (a bug in that
        // filter): degrade to the seed content rather than propagating a
        // non-string value into the template.
        None => context.insert("RIKU_NGINX_INCLUDE_CONTENT", &content),
    }
    Ok(())
}

#[cfg(test)]
#[path = "app_snippet_tests.rs"]
mod tests;
