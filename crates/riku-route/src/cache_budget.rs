//! A cap on the disk all apps' proxy caches may fill together.
//!
//! nginx bounds one cache by its `max_size`, and the gate bounds that per
//! app, but a deploy user with many apps could still ask for a sum larger
//! than the disk. The helper adds up the `max_size` of the configs it has
//! already written, which are its own output, and refuses a route that would
//! pass [`TOTAL_CACHE_GB`].

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use riku_container::types::AppName;

/// The most disk, in GiB, all proxy caches on a host may be allowed.
pub(crate) const TOTAL_CACHE_GB: u32 = 100;

const CACHE_DIRECTIVE: &str = "proxy_cache_path ";
const CACHE_FORM: &str = " levels=1:2 keys_zone=";
const MAX_SIZE_KEY: &str = " max_size=";

/// The GiB of a line in exactly the form `nginx_cache_path.conf.tera`
/// renders: at column 0, ending in `max_size=<n>g;`. Anything else, such as
/// a line a snippet or header might carry, counts for nothing.
fn line_gb(line: &str) -> Option<u32> {
    if !line.starts_with(CACHE_DIRECTIVE) || !line.contains(CACHE_FORM) {
        return None;
    }
    line.rsplit_once(MAX_SIZE_KEY)
        .and_then(|(_, size)| size.strip_suffix("g;"))
        .and_then(|digits| digits.parse().ok())
}

fn allowed_gb(config: &str) -> u32 {
    config
        .lines()
        .filter_map(line_gb)
        .fold(0, u32::saturating_add)
}

fn other_apps_gb(site_root: &Path, app: &AppName) -> Result<u32> {
    let entries = match fs::read_dir(site_root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(anyhow!("cannot read {}: {e}", site_root.display())),
    };
    let own = format!("{app}.conf");
    let mut total = 0u32;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".conf") || name == own {
            continue;
        }
        match fs::read_to_string(entry.path()) {
            Ok(config) => total = total.saturating_add(allowed_gb(&config)),
            Err(e) => return Err(anyhow!("cannot read {}: {e}", entry.path().display())),
        }
    }
    Ok(total)
}

/// Refuse `wanted_gb` for `app` when the other apps' caches plus it pass the
/// total.
pub(crate) fn within_budget(site_root: &Path, app: &AppName, wanted_gb: u32) -> Result<()> {
    let others = match other_apps_gb(site_root, app) {
        Ok(others) => others,
        Err(e) => return Err(anyhow!("cannot add up the proxy caches: {e}")),
    };
    if others.saturating_add(wanted_gb) > TOTAL_CACHE_GB {
        return Err(anyhow!(
            "the proxy cache of '{app}' ({wanted_gb} GB) would bring all caches to more than {TOTAL_CACHE_GB} GB; other apps already hold {others} GB"
        ));
    }
    Ok(())
}
