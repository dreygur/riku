//! Root init's part of the Cloudflare ACL: fetch the address ranges and
//! write `/etc/riku/cloudflare.conf`, root owned `0644`.
//!
//! Security: this is the only place the ranges are fetched. The file is
//! written by root into a directory root owns that nobody else can write
//! (see [`check_write_dir`]), through a temp file and a rename, so the deploy
//! user can neither replace it nor plant a symlink in its place. The router
//! helper includes this file and never fetches. A failed fetch leaves the
//! previous file as it was.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::CLOUDFLARE_CONF;
use crate::nginx::{cloudflare_allow_config, fetch_cloudflare_url};
use crate::util::echo;

use super::nginx_host::{check_write_dir, ROOT_UID};

const FILE_MODE: u32 = 0o644;
const DIR_MODE: u32 = 0o755;

fn ensure_parent(dir: &Path, owner: u32) -> Result<()> {
    if !dir.exists() {
        if let Err(e) = fs::create_dir_all(dir) {
            return Err(anyhow!("cannot create {}: {e}", dir.display()));
        }
        if let Err(e) = fs::set_permissions(dir, fs::Permissions::from_mode(DIR_MODE)) {
            return Err(anyhow!("cannot chmod {}: {e}", dir.display()));
        }
    }
    check_write_dir(dir, owner)
}

/// Fetch the ranges through `fetch` and write them to `file`, whose parent
/// directory must belong to `owner`.
pub(super) fn write_ranges(
    file: &Path,
    owner: u32,
    fetch: impl Fn(&str) -> Result<String>,
) -> Result<()> {
    let Some(dir) = file.parent() else {
        return Err(anyhow!("{} has no directory", file.display()));
    };
    if let Err(e) = ensure_parent(dir, owner) {
        return Err(anyhow!("cannot prepare {}: {e}", dir.display()));
    }
    let contents = match cloudflare_allow_config(fetch) {
        Ok(contents) => contents,
        Err(e) => return Err(anyhow!("cannot fetch the Cloudflare ranges: {e}")),
    };
    if let Err(e) = crate::util::write_atomic(file, contents.as_bytes()) {
        return Err(anyhow!("cannot write {}: {e}", file.display()));
    }
    match fs::set_permissions(file, fs::Permissions::from_mode(FILE_MODE)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", file.display())),
    }
}

/// Write `/etc/riku/cloudflare.conf` from Cloudflare's published ranges. A
/// host without network access still finishes init: the problem is reported
/// as a warning, and doctor keeps warning until the file exists.
pub fn setup_cloudflare_ranges() -> Result<()> {
    match write_ranges(Path::new(CLOUDFLARE_CONF), ROOT_UID, fetch_cloudflare_url) {
        Ok(()) => echo(&format!("      {CLOUDFLARE_CONF} written"), "green"),
        Err(e) => echo(
            &format!(
                "      warning: no Cloudflare ranges ({e}); sites with RIKU_NGINX_CLOUDFLARE_ACL are refused until you re-run: sudo riku init"
            ),
            "yellow",
        ),
    }
    Ok(())
}

#[cfg(test)]
#[path = "cloudflare_ranges_tests.rs"]
mod tests;
