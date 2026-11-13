//! The fallback domain's certificate on nginx: issue it once the site
//! serves the ACME challenge over HTTP, remember how it went, and after a
//! failure wait an hour before asking Let's Encrypt again, so repeated pushes
//! and rollouts stay inside its failed validation limit.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use riku_nginx::SingleCert;

use crate::config::RikuPaths;
use crate::router::routing_state::{with_routing_lock, Entry, RoutingState, TlsState};
use crate::util::display;

const RETRY_AFTER_SECS: u64 = 3600;
const PAIR_SUFFIXES: [&str; 3] = ["key", "crt", "fullchain.crt"];

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

fn last_failure(paths: &RikuPaths, app: &str) -> Option<u64> {
    let state = RoutingState::load(paths).ok();
    match state
        .as_ref()
        .and_then(|state| state.entry(app))
        .and_then(|entry| entry.tls)
    {
        Some(TlsState::FailedAt(at)) => Some(at),
        _ => None,
    }
}

fn record(paths: &RikuPaths, app: &str, tls: TlsState, cert_for: Option<&str>) {
    let saved = with_routing_lock(paths, |state| {
        if let Some(entry) = state.entry(app).cloned() {
            state.set_entry(
                app,
                Entry {
                    tls: Some(tls),
                    cert_for: cert_for.map(str::to_string).or(entry.cert_for.clone()),
                    ..entry
                },
            );
        }
        Ok(())
    });
    if let Err(e) = saved {
        tracing::warn!("cannot record the certificate outcome of '{app}': {e:#}");
    }
}

/// Record that the pair in `nginx_root` is for `domain` before acme.sh
/// writes it. acme.sh runs `riku router-renew` while the issue is still
/// going, and a pair with no record of its name would look stale there and
/// be deleted.
fn claim_pair(paths: &RikuPaths, app: &str, domain: &str) {
    let saved = with_routing_lock(paths, |state| {
        if let Some(entry) = state.entry(app).cloned() {
            state.set_entry(
                app,
                Entry {
                    cert_for: Some(domain.to_string()),
                    ..entry
                },
            );
        }
        Ok(())
    });
    if let Err(e) = saved {
        tracing::warn!("cannot record the certificate name of '{app}': {e:#}");
    }
}

/// [`fallback_certificate`] with the issuer and the clock given.
pub(super) fn fallback_certificate_with(
    issue: impl Fn(&str, &str, &RikuPaths) -> Result<SingleCert>,
    app: &str,
    domain: &str,
    paths: &RikuPaths,
    at: u64,
) -> Option<TlsState> {
    if let Some(failed) = last_failure(paths, app) {
        if at.saturating_sub(failed) < RETRY_AFTER_SECS {
            return Some(TlsState::FailedAt(failed));
        }
    }
    claim_pair(paths, app, domain);
    let outcome = match issue(app, domain, paths) {
        Ok(SingleCert::Ready) => TlsState::Issued,
        Ok(SingleCert::Failed) => TlsState::FailedAt(at),
        Ok(SingleCert::NoAcme(acme_sh)) => {
            display::warn(&format!(
                "No certificate for {domain}: acme.sh is not installed at {}. Install it as the deploy user: cd ~ && curl -fsSL https://get.acme.sh | sh",
                acme_sh.display()
            ));
            return None;
        }
        Err(e) => {
            display::warn(&format!("Certificate for {domain} failed: {e:#}"));
            TlsState::FailedAt(at)
        }
    };
    let covers = (outcome == TlsState::Issued).then_some(domain);
    record(paths, app, outcome, covers);
    Some(outcome)
}

/// Issue `app`'s fallback certificate for `domain` unless a recent attempt
/// failed. Never fails the deploy: without a certificate the app stays on
/// plain HTTP. `None` means nothing was tried, because acme.sh is missing,
/// so the next deploy tries again.
pub(super) fn fallback_certificate(app: &str, domain: &str, paths: &RikuPaths) -> Option<TlsState> {
    fallback_certificate_with(riku_nginx::issue_single, app, domain, paths, now())
}

/// Whether the pair in `nginx_root` covers another name than the one the
/// app is served under now. `cert_for` is the fallback name riku issued the
/// pair for (`None`: an own domain pair or none); `fallback` is the
/// fallback name in use (`None`: the app's own domains).
pub(super) fn pair_is_stale(cert_for: Option<&str>, fallback: Option<&str>) -> bool {
    match (cert_for, fallback) {
        (None, None) => false,
        (Some(issued), Some(now)) => issued != now,
        _ => true,
    }
}

/// Delete `app`'s pair when it covers another name than `fallback`, so
/// nginx never serves a certificate for the wrong name and a fresh one is
/// issued. Clears the record of the old one. A pair the owner supplied (see
/// `riku certs set`) is never deleted.
pub(super) fn drop_stale_pair(app: &str, fallback: Option<&str>, paths: &RikuPaths) {
    if crate::config::own_cert::has_own_cert(paths, app) {
        return;
    }
    let entry = RoutingState::load(paths)
        .ok()
        .and_then(|state| state.entry(app).cloned());
    let cert_for = entry.as_ref().and_then(|entry| entry.cert_for.as_deref());
    let files: Vec<_> = PAIR_SUFFIXES
        .iter()
        .map(|suffix| paths.nginx_root.join(format!("{app}.{suffix}")))
        .collect();
    if !pair_is_stale(cert_for, fallback) || !files.iter().any(|file| file.exists()) {
        return;
    }
    for file in &files {
        if let Err(e) = std::fs::remove_file(file) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!("cannot remove {}: {e}", file.display());
            }
        }
    }
    let cleared = with_routing_lock(paths, |state| {
        if let Some(entry) = state.entry(app).cloned() {
            state.set_entry(
                app,
                Entry {
                    tls: None,
                    cert_for: None,
                    ..entry
                },
            );
        }
        Ok(())
    });
    if let Err(e) = cleared {
        tracing::warn!("cannot clear the certificate record of '{app}': {e:#}");
    }
}

/// Record that Caddy issues `app`'s fallback certificate itself.
pub(super) fn mark_caddy_tls(app: &str, paths: &RikuPaths) {
    record(paths, app, TlsState::ByCaddy, None);
}

#[cfg(test)]
#[path = "fallback_tls_tests.rs"]
mod tests;
