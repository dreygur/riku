//! The `[router]` section of `settings.toml`.
//!
//! ```toml
//! [router]
//! cache_total_gb = 10
//! ```
//!
//! `cache_total_gb` is the most disk, in GiB, that every app's nginx proxy
//! cache may be allowed together. The router helper reads it when it starts,
//! so a change takes effect on its next start. The deploy user can edit the
//! file, so this is a guard against runaway configs, not a boundary against
//! the deploy user.

use std::path::Path;

use anyhow::{anyhow, Result};

/// Default for `router.cache_total_gb`.
pub const DEFAULT_CACHE_TOTAL_GB: u32 = 10;

/// The largest value accepted for `router.cache_total_gb`, 1 PiB, so the sum
/// of cache sizes cannot overflow.
const MAX_CACHE_TOTAL_GB: u32 = 1_048_576;

/// Router wide limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouterSettings {
    /// GiB all proxy caches together may be allowed.
    pub cache_total_gb: u32,
}

impl Default for RouterSettings {
    fn default() -> Self {
        RouterSettings {
            cache_total_gb: DEFAULT_CACHE_TOTAL_GB,
        }
    }
}

fn cache_total_at(value: &toml::Value, file: &Path) -> Result<u32> {
    let parsed = value
        .as_integer()
        .filter(|gb| (1..=i64::from(MAX_CACHE_TOTAL_GB)).contains(gb))
        .and_then(|gb| u32::try_from(gb).ok());
    match parsed {
        Some(gb) => Ok(gb),
        None => Err(anyhow!(
            "{}: router.cache_total_gb must be a whole number from 1 to {MAX_CACHE_TOTAL_GB}",
            file.display()
        )),
    }
}

/// Read the `[router]` table.
pub(crate) fn router_settings(table: &toml::Table, file: &Path) -> Result<RouterSettings> {
    let mut router = RouterSettings::default();
    for (key, value) in table {
        match key.as_str() {
            "cache_total_gb" => {
                router.cache_total_gb = match cache_total_at(value, file) {
                    Ok(gb) => gb,
                    Err(e) => return Err(anyhow!("{e}")),
                };
            }
            _ => return Err(anyhow!("{}: unknown key router.{key}", file.display())),
        }
    }
    Ok(router)
}
