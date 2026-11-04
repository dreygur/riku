//! How the system nginx finds riku's sites: one root-owned file,
//! `nginx_include_file`, holding a single `include <nginx_root>/*.conf;`.
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

fn breaks_directive(c: char) -> bool {
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

/// The full text of the include file.
pub fn include_file_contents(paths: &RikuPaths) -> Result<String> {
    match include_line(paths) {
        Ok(line) => Ok(format!(
            "# Written by riku init. Loads the deploy user's site configs.\n{line}\n"
        )),
        Err(e) => Err(e),
    }
}

/// Write the include file atomically, creating its directory.
#[allow(clippy::question_mark)]
pub fn write_include_file(paths: &RikuPaths) -> Result<()> {
    let contents = match include_file_contents(paths) {
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
