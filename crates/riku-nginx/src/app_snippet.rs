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
//!
//! Installed filter plugins may then rewrite the text through the
//! `nginx.include_content` chain. Checks 2 and 3 run again on the chain's
//! output, so a filter cannot add what the app's file may not contain.

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

/// The filter chain the snippet runs through before it reaches the template.
const INCLUDE_FILTER: &str = "nginx.include_content";

/// Refuse snippet text that does not fit the grammar or uses a directive
/// outside the allowlist, and return the directive names it uses. `source`
/// names where the text came from in the error.
fn check_snippet(content: &str, source: impl std::fmt::Display) -> Result<Vec<String>> {
    let parsed = match statements(content) {
        Ok(parsed) => parsed,
        Err(reason) => return Err(anyhow!("{source} {reason}")),
    };
    let names: Vec<String> = parsed
        .iter()
        .filter_map(|statement| statement.split_whitespace().next())
        .map(str::to_string)
        .collect();
    match names
        .iter()
        .find(|name| !ALLOWED_DIRECTIVES.contains(&name.as_str()))
    {
        Some(name) => Err(anyhow!(
            "{source} uses `{name}`, which a {SNIPPET_KEY} snippet may not; allowed: {}",
            ALLOWED_DIRECTIVES.join(", ")
        )),
        None => Ok(names),
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
    match check_snippet(&content, path.display()) {
        Ok(_) => Ok(content),
        Err(e) => Err(e),
    }
}

/// Read the app's checked snippet as the seed value, run it through the
/// `nginx.include_content` filter chain (`PLUGIN_PROTOCOL.md` §7.3: lets an
/// installed plugin augment the generated config without replacing it), and
/// insert the result. Always inserts the key, even as an empty string, so
/// it's never simply absent from the template context.
///
/// The chain's output gets the same grammar and allowlist check as the
/// app's file. Filter plugins sit in the deploy user's tree, so without it
/// a filter could put back any directive the app's file is refused for. A
/// chain that returns a non-string or refused text fails the deploy.
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

    let filtered =
        riku_plugins::FilterBus::new(paths).apply(INCLUDE_FILTER, serde_json::json!(content));
    let Some(filtered) = filtered.as_str() else {
        return Err(anyhow!(
            "the {INCLUDE_FILTER} filter chain returned {filtered}, not a string; refusing the nginx config"
        ));
    };
    let directives = match check_snippet(filtered, format!("{INCLUDE_FILTER} filter output")) {
        Ok(directives) => directives,
        Err(e) => return Err(e),
    };
    context.insert("RIKU_NGINX_INCLUDE_CONTENT", filtered);
    // The templates leave out their own line for each directive the snippet
    // sets: nginx refuses a second `client_max_body_size` or `gzip` in one
    // server block.
    context.insert("RIKU_NGINX_SNIPPET_DIRECTIVES", &directives);
    Ok(())
}

#[cfg(test)]
#[path = "app_snippet_tests.rs"]
mod tests;
