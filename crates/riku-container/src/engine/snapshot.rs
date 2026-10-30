//! Reading a set of containers while others may be removed under us.
//!
//! A rolling deploy removes old copies while the supervisor polls, so a
//! container can vanish between listing it and asking for its state. The
//! engine then fails the whole batch command. [`fetch_present`] retries each
//! id on its own and drops only ids that a fresh listing of riku containers no
//! longer contains. If the listing fails, or still shows the id, the original
//! error is returned: a daemon failure is never mistaken for a removed
//! container. No engine error text is inspected.

use std::slice;

use anyhow::{anyhow, Result};

use super::{ContainerId, Engine};

/// The error to report for `id`, or `None` when `id` is gone and may be dropped.
fn error_unless_gone(
    engine: &Engine,
    listed: &mut Option<Vec<ContainerId>>,
    id: &ContainerId,
    error: anyhow::Error,
) -> Option<anyhow::Error> {
    if listed.is_none() {
        match engine.list_riku_containers() {
            Ok(all) => *listed = Some(all),
            Err(list_error) => {
                return Some(anyhow!(
                    "{error}; listing containers also failed: {list_error}"
                ))
            }
        }
    }
    if listed.iter().flatten().any(|present| present == id) {
        Some(error)
    } else {
        None
    }
}

/// Run `run` for all of `ids`, falling back to one id at a time when the batch
/// fails, and parse each successful output with `parse`.
///
/// A parse failure is returned as is, never retried.
pub(super) fn fetch_present<T>(
    engine: &Engine,
    ids: &[ContainerId],
    run: impl Fn(&[ContainerId]) -> Result<String>,
    parse: impl Fn(&str) -> Result<Vec<T>>,
) -> Result<Vec<T>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    if let Ok(output) = run(ids) {
        return parse(&output);
    }
    let mut found = Vec::new();
    let mut listed = None;
    for id in ids {
        match run(slice::from_ref(id)) {
            Ok(output) => {
                if let Err(e) = parse(&output).map(|items| found.extend(items)) {
                    return Err(anyhow!("reading containers one at a time: {e}"));
                }
            }
            Err(error) => {
                if let Some(e) = error_unless_gone(engine, &mut listed, id, error) {
                    return Err(e);
                }
            }
        }
    }
    Ok(found)
}
