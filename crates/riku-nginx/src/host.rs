//! How the system nginx finds riku's sites: one root-owned file,
//! `nginx_include_file`, holding a single `include /etc/riku/nginx/*.conf;`
//! and, on a host with no default server for port 80, riku's catch-all.
//!
//! The router helper writes site configs only into `/etc/riku/nginx`. Root
//! writes the include file once at init and nothing else under `/etc/nginx`.
//!
//! Security: the include file is root-owned and nginx's master parses it as
//! root. The path in it is a constant, so nothing the deploy user controls
//! reaches the directive, and nginx loads no file from the deploy user's tree.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::config::{RikuPaths, NGINX_SITE_ROOT};
use crate::default_server::CATCH_ALL_SERVER;

/// The single `include` line that loads every site config the router helper
/// rendered into [`NGINX_SITE_ROOT`].
pub fn include_line() -> String {
    format!("include {NGINX_SITE_ROOT}/*.conf;")
}

/// The full text of the include file, with riku's catch-all server when
/// `catch_all` is set (see [`crate::needs_catch_all`]).
pub fn include_file_contents(catch_all: bool) -> String {
    let mut contents = format!(
        "# Written by riku init. Loads the site configs the router helper renders.\n{}\n",
        include_line()
    );
    if catch_all {
        contents.push_str("\n# No other server is the default for port 80.\n");
        contents.push_str(CATCH_ALL_SERVER);
    }
    contents
}

/// Write the include file atomically, replacing an older include of the
/// deploy user's own nginx directory.
pub fn write_include_file(paths: &RikuPaths, catch_all: bool) -> Result<()> {
    let file = &paths.nginx_include_file;
    match crate::util::write_atomic(file, include_file_contents(catch_all).as_bytes()) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot write {}: {e}", file.display())),
    }
}

/// The site config the router helper writes for `app`:
/// `/etc/riku/nginx/<app>.conf`. `app` must already be validated.
pub fn site_path(app: &str) -> PathBuf {
    Path::new(NGINX_SITE_ROOT).join(format!("{app}.conf"))
}

/// Whether the include file exists and loads [`NGINX_SITE_ROOT`].
pub fn includes_riku_sites(paths: &RikuPaths) -> bool {
    let line = include_line();
    match fs::read_to_string(&paths.nginx_include_file) {
        Ok(text) => text.lines().any(|l| l.trim() == line),
        Err(_) => false,
    }
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
