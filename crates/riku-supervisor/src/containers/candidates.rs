//! Choosing which exited copies to restart and recording how it went.

use std::time::Instant;

use riku_config::RikuPaths;
use riku_container::engine::{ContainerId, ContainerInfo, ContainerState, Engine};

use super::listing::{merge_inspected, Known};
use super::restart_policy::{should_restart, RestartPolicyLabel};
use super::restarts::Restarts;
use super::tally::RestartTally;
use crate::health::actions::{RestartOutcome, SharedActions};

fn label_policy(info: &ContainerInfo) -> RestartPolicyLabel {
    match info.restart.as_deref() {
        Some(raw) => match RestartPolicyLabel::try_from(raw) {
            Ok(policy) => policy,
            Err(_) => {
                tracing::warn!(name = %info.name, "unreadable restart label, not restarting");
                RestartPolicyLabel::No
            }
        },
        None => RestartPolicyLabel::No,
    }
}

fn exit_code(info: &ContainerInfo) -> Option<i32> {
    match (&info.state, &info.app, &info.service, &info.copy) {
        (ContainerState::Exited { code }, Some(_), Some(_), Some(_)) => Some(*code),
        _ => None,
    }
}

/// Apply finished restarts to the tally. A copy that was restarted, whose
/// restart failed, or that was already gone, is gone from the engine, so its
/// old id is forgotten; only real attempts count. A busy copy is untouched
/// and tried again on the next tick.
pub(crate) fn collect_reports(
    restarts: &mut Restarts,
    tally: &mut RestartTally,
    known: &mut Known,
    now: Instant,
) {
    for report in restarts.collect() {
        match report.outcome {
            Ok(RestartOutcome::Busy) => {}
            Ok(RestartOutcome::Gone) => {
                known.remove(&report.id);
            }
            Ok(RestartOutcome::Restarted) => {
                tally.record_restart(&report.name, now);
                known.remove(&report.id);
            }
            Err(e) => {
                tracing::warn!(name = %report.name, "restart failed: {e:#}");
                tally.record_restart(&report.name, now);
                known.remove(&report.id);
            }
        }
    }
}

/// Exited copies whose policy, restart count and backoff allow a restart now.
pub(crate) fn find_candidates(
    known: &Known,
    tally: &RestartTally,
    restarts: &Restarts,
    now: Instant,
) -> Vec<ContainerId> {
    known
        .values()
        .filter(|info| !restarts.is_in_flight(&info.name))
        .filter(|info| match exit_code(info) {
            Some(code) => should_restart(&label_policy(info), code, tally.count(&info.name)),
            None => false,
        })
        .filter(|info| tally.due(&info.name, now))
        .map(|info| info.id.clone())
        .collect()
}

/// Inspect the candidates once more and dispatch those still exited.
pub(crate) fn dispatch_restarts(
    engine: &Engine,
    paths: &RikuPaths,
    actions: &SharedActions,
    restarts: &mut Restarts,
    known: &mut Known,
    candidates: Vec<ContainerId>,
) {
    if candidates.is_empty() {
        return;
    }
    match engine.inspect(&candidates) {
        Ok(infos) => merge_inspected(known, &candidates, infos),
        Err(e) => {
            tracing::warn!(
                "cannot inspect {} restart candidates: {e}",
                candidates.len()
            );
            return;
        }
    }
    for id in &candidates {
        let info = match known.get(id) {
            Some(info) if exit_code(info).is_some() => info,
            _ => continue,
        };
        if let Err(e) = restarts.dispatch(paths, actions, info) {
            tracing::warn!(name = %info.name, "cannot start the restart: {e}");
        }
    }
}
