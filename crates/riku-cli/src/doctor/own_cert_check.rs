//! `riku doctor` check for the certificates apps serve from their own
//! pair (`riku certs set`). riku never renews these, so a warning 14 days
//! before the end date is the only reminder the owner gets.
//!
//! Read-only: it lists the `<app>.own-cert` markers in `nginx_root` and
//! reads each leaf certificate.

use std::fs;

use crate::config::own_cert::OWN_CERT_SUFFIX;
use crate::config::RikuPaths;
use crate::own_cert::info::{self, Expiry};
use crate::own_cert::store;

use super::Check;

const NAME: &str = "own certificate";

fn marked_apps(paths: &RikuPaths) -> Vec<String> {
    let Ok(entries) = fs::read_dir(&paths.nginx_root) else {
        return Vec::new();
    };
    let mut apps: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.strip_suffix(&format!(".{OWN_CERT_SUFFIX}"))
                .map(str::to_string)
        })
        .collect();
    apps.sort();
    apps
}

fn check_app(paths: &RikuPaths, app: &str, now: i64) -> Check {
    let Some(leaf) = store::leaf_pem(paths, app) else {
        return Check::fail(
            NAME,
            format!("{app}: no certificate file; run `riku certs set {app}`"),
        );
    };
    let found = match info::read(&leaf) {
        Ok(found) => found,
        Err(e) => return Check::fail(NAME, format!("{app}: {e:#}")),
    };
    match info::classify(found.expires_at, now) {
        Expiry::Expired => Check::fail(
            NAME,
            format!("{app}: expired; run `riku certs set {app}` with a new certificate"),
        ),
        Expiry::Soon { days } => Check::warn(
            NAME,
            format!(
                "{app}: expires in {days} days; riku does not renew it, run `riku certs set {app}`"
            ),
        ),
        Expiry::Valid { days } => Check::ok(NAME, format!("{app}: valid for {days} more days")),
    }
}

/// One check per app that serves its own certificate, judged at `now`
/// (seconds since the epoch).
pub fn own_certificates_at(paths: &RikuPaths, now: i64) -> Vec<Check> {
    marked_apps(paths)
        .iter()
        .map(|app| check_app(paths, app, now))
        .collect()
}

/// [`own_certificates_at`] with the clock.
pub fn own_certificates(paths: &RikuPaths) -> Vec<Check> {
    own_certificates_at(paths, info::now())
}

#[cfg(test)]
#[path = "own_cert_check_tests.rs"]
mod tests;
