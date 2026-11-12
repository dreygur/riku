//! The process kinds a Procfile app's scale controls offer.

use std::collections::{BTreeSet, HashMap};

use crate::config::RikuPaths;
use crate::deploy::workers::is_scalable_kind;
use crate::util::parse_settings;

use super::types::{UnitKind, WorkerStateEntry};

/// The kinds the scale API takes for a Procfile app: the scalable kinds in
/// its SCALING file (a kind scaled to 0 included) and of its process
/// workers, sorted, without repeats. An unreadable SCALING file leaves only
/// the workers' kinds.
pub(super) fn process_kinds(
    paths: &RikuPaths,
    app: &str,
    workers: &[WorkerStateEntry],
) -> Vec<String> {
    let scaling = paths.env_root.join(app).join("SCALING");
    let counted = parse_settings(&scaling, &mut HashMap::new()).unwrap_or_default();
    let worker_kinds = workers
        .iter()
        .filter(|w| w.unit == UnitKind::Process)
        .map(|w| w.kind.clone());
    let kinds: BTreeSet<String> = counted
        .into_keys()
        .chain(worker_kinds)
        .filter(|kind| is_scalable_kind(kind))
        .collect();
    kinds.into_iter().collect()
}
