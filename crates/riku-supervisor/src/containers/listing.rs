//! Keeping the watch's view of riku containers current: full listings,
//! event driven inspects and stats samples.
//!
//! The engine never has two containers with one name, so a container
//! inspected under a name replaces any other id remembered for that name.

use std::collections::{HashMap, HashSet};

use riku_container::engine::{ContainerId, ContainerInfo, ContainerState, ContainerStats, Engine};

use super::Feed;

pub(crate) type Known = HashMap<ContainerId, ContainerInfo>;

pub(crate) fn is_running(state: &ContainerState) -> bool {
    matches!(
        state,
        ContainerState::Running | ContainerState::Starting | ContainerState::Unhealthy
    )
}

pub(crate) fn running_ids(known: &Known) -> HashSet<ContainerId> {
    known
        .values()
        .filter(|info| is_running(&info.state))
        .map(|info| info.id.clone())
        .collect()
}

pub(crate) fn remember(known: &mut Known, info: ContainerInfo) {
    known.retain(|id, other| other.name != info.name || *id == info.id);
    known.insert(info.id.clone(), info);
}

/// Merge a fresh inspect of `asked` into `known`: ids the inspect left out
/// were removed from the engine and are forgotten.
pub(crate) fn merge_inspected(known: &mut Known, asked: &[ContainerId], infos: Vec<ContainerInfo>) {
    let present: HashSet<ContainerId> = infos.iter().map(|info| info.id.clone()).collect();
    for id in asked {
        if !present.contains(id) {
            known.remove(id);
        }
    }
    for info in infos {
        remember(known, info);
    }
}

/// Replace `known` with a full listing. On an engine error `known` is kept
/// and false is returned.
pub(crate) fn take_listing(engine: &Engine, known: &mut Known) -> bool {
    let ids = match engine.list_riku() {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!("cannot list riku containers: {e}");
            return false;
        }
    };
    let infos = match engine.inspect(&ids) {
        Ok(infos) => infos,
        Err(e) => {
            tracing::warn!("cannot inspect riku containers: {e}");
            return false;
        }
    };
    known.clear();
    for info in infos {
        remember(known, info);
    }
    true
}

pub(crate) fn sample_stats(
    engine: &Engine,
    known: &Known,
    samples: &mut HashMap<String, ContainerStats>,
) {
    let running: Vec<ContainerId> = running_ids(known).into_iter().collect();
    if running.is_empty() {
        samples.clear();
        return;
    }
    match engine.stats(&running) {
        Ok(fresh) => {
            samples.clear();
            for sample in fresh {
                samples.insert(sample.id.clone(), sample);
            }
        }
        Err(e) => tracing::warn!("cannot sample container stats: {e}"),
    }
}

/// Inspect the containers the event stream named since the last call. A
/// closed stream turns the feed into polling. True when `known` was updated.
pub(crate) fn drain_events(engine: &Engine, feed: &mut Feed, known: &mut Known) -> bool {
    let drained = match feed {
        Feed::Events(stream) => stream.drain(),
        Feed::Polling => return false,
    };
    if drained.closed {
        tracing::warn!("container event stream closed, polling every 30 s instead");
        *feed = Feed::Polling;
    }
    let ids: Vec<ContainerId> = drained
        .ids
        .into_iter()
        .collect::<HashSet<ContainerId>>()
        .into_iter()
        .collect();
    if ids.is_empty() {
        return false;
    }
    match engine.inspect(&ids) {
        Ok(infos) => {
            merge_inspected(known, &ids, infos);
            true
        }
        Err(e) => {
            tracing::warn!("cannot inspect {} event containers: {e}", ids.len());
            false
        }
    }
}
