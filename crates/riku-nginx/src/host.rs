//! How the system nginx finds riku's sites: one root-owned file,
//! `nginx_include_file`, holding a single `include <nginx_root>/*.conf;`
//! and, on a host with no default server for port 80, riku's catch-all.
//!
//! riku writes site configs only into its own `nginx_root`. Root writes the
//! include file once at init and nothing else under `/etc/nginx`.
//!
//! Security: the include file is root-owned and nginx's master parses it as
//! root, so the path written into it is refused unless it is absolute and
//! free of anything that could end the directive or start a new one
//! (whitespace, control characters, `;`, `{`, `}`, `#`, quotes, `$`).

use std::fs;

use anyhow::{anyhow, Result};

use crate::config::RikuPaths;
use crate::default_server::CATCH_ALL_SERVER;

/// Whether `c` could end an nginx directive or start another inside a value
/// written unquoted into a config file.
pub fn breaks_directive(c: char) -> bool {
    c.is_whitespace() || c.is_control() || matches!(c, ';' | '{' | '}' | '#' | '"' | '\'' | '$')
}

/// The single `include` line that loads every site config in `nginx_root`.
pub fn include_line(paths: &RikuPaths) -> Result<String> {
    let dir = &paths.nginx_root;
    let Some(text) = dir.to_str() else {
        return Err(anyhow!("nginx directory {dir:?} is not valid UTF-8"));
    };
    if !dir.is_absolute() || text.contains(breaks_directive) {
        return Err(anyhow!(
            "refusing nginx directory {dir:?}: it must be absolute with no \
             whitespace, quotes or nginx syntax characters"
        ));
    }
    Ok(format!("include {text}/*.conf;"))
}

/// The full text of the include file, with riku's catch-all server when
/// `catch_all` is set (see [`crate::needs_catch_all`]).
#[allow(clippy::question_mark)]
pub fn include_file_contents(paths: &RikuPaths, catch_all: bool) -> Result<String> {
    let line = match include_line(paths) {
        Ok(line) => line,
        Err(e) => return Err(e),
    };
    let mut contents =
        format!("# Written by riku init. Loads the deploy user's site configs.\n{line}\n");
    if catch_all {
        contents.push_str("\n# No other server is the default for port 80.\n");
        contents.push_str(CATCH_ALL_SERVER);
    }
    Ok(contents)
}

/// Write the include file atomically, creating its directory.
#[allow(clippy::question_mark)]
pub fn write_include_file(paths: &RikuPaths, catch_all: bool) -> Result<()> {
    let contents = match include_file_contents(paths, catch_all) {
        Ok(contents) => contents,
        Err(e) => return Err(e),
    };
    let file = &paths.nginx_include_file;
    match crate::util::write_atomic(file, contents.as_bytes()) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot write {}: {e}", file.display())),
    }
}

/// Whether the include file exists and loads `nginx_root`.
pub fn includes_riku_sites(paths: &RikuPaths) -> bool {
    let Ok(line) = include_line(paths) else {
        return false;
    };
    match fs::read_to_string(&paths.nginx_include_file) {
        Ok(text) => text.lines().any(|l| l.trim() == line),
        Err(_) => false,
    }
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
