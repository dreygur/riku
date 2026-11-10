//! The fallback domain's certificate on nginx: issue it once the site
//! serves the ACME challenge over HTTP, remember how it went, and after a
//! failure wait an hour before asking Let's Encrypt again, so repeated pushes
//! and rollouts stay inside its failed validation limit.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::config::RikuPaths;
use crate::router::routing_state::{with_routing_lock, RoutingState, TlsState};
use crate::util::echo;

const RETRY_AFTER_SECS: u64 = 3600;

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

fn record(paths: &RikuPaths, app: &str, tls: TlsState) {
    let saved = with_routing_lock(paths, |state| {
        if let Some(entry) = state.entry(app).cloned() {
            state.set_entry(
                app,
                crate::router::routing_state::Entry {
                    tls: Some(tls),
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

/// [`fallback_certificate`] with the issuer and the clock given.
pub(super) fn fallback_certificate_with(
    issue: impl Fn(&str, &str, &RikuPaths) -> Result<bool>,
    app: &str,
    domain: &str,
    paths: &RikuPaths,
    at: u64,
) -> TlsState {
    if let Some(failed) = last_failure(paths, app) {
        if at.saturating_sub(failed) < RETRY_AFTER_SECS {
            return TlsState::FailedAt(failed);
        }
    }
    let outcome = match issue(app, domain, paths) {
        Ok(true) => TlsState::Issued,
        Ok(false) => TlsState::FailedAt(at),
        Err(e) => {
            echo(
                &format!("-----> Certificate for {domain} failed: {e:#}"),
                "yellow",
            );
            TlsState::FailedAt(at)
        }
    };
    record(paths, app, outcome);
    outcome
}

/// Issue `app`'s fallback certificate for `domain` unless a recent attempt
/// failed. Never fails the deploy: without a certificate the app stays on
/// plain HTTP.
pub(super) fn fallback_certificate(app: &str, domain: &str, paths: &RikuPaths) -> TlsState {
    fallback_certificate_with(riku_nginx::issue_single, app, domain, paths, now())
}

/// Record that Caddy issues `app`'s fallback certificate itself.
pub(super) fn mark_caddy_tls(app: &str, paths: &RikuPaths) {
    record(paths, app, TlsState::ByCaddy);
}

#[cfg(test)]
#[path = "fallback_tls_tests.rs"]
mod tests;
