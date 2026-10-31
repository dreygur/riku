//! Adding and removing copies of a service without touching the others.

use anyhow::Result;
use riku_container::types::CopyIndex;
use riku_util::backend::Backend;

use super::replace::{CopyRuntime, LiveCopy, RouteSink};

fn backends_of(copies: &[LiveCopy]) -> Vec<Backend> {
    copies.iter().filter_map(|c| c.backend.clone()).collect()
}

fn lowest_free(copies: &[LiveCopy]) -> CopyIndex {
    let mut n = 1;
    while copies.iter().any(|c| c.copy.get() == n) {
        n += 1;
    }
    CopyIndex::new(n).expect("copy numbers start at 1")
}

/// Starts a copy at the lowest free index and waits for it. An unhealthy copy
/// is discarded and its error returned; `copies` is not changed.
pub(super) fn start_healthy(
    runtime: &mut impl CopyRuntime,
    copies: &[LiveCopy],
) -> Result<LiveCopy> {
    let index = lowest_free(copies);
    let fresh = match runtime.start(index) {
        Ok(fresh) => fresh,
        Err(e) => return Err(e.context(format!("could not start copy {index}"))),
    };
    if let Err(e) = runtime.wait_healthy(&fresh) {
        runtime.discard(&fresh);
        return Err(e.context(format!("copy {index} did not become healthy")));
    }
    Ok(fresh)
}

/// Routes to `fresh`, already pushed onto `copies`. When the router cannot
/// confirm, the copy stays running: it may already be serving, and the next
/// deploy replaces it.
pub(super) fn publish_added(
    routes: &mut impl RouteSink,
    copies: &[LiveCopy],
    fresh: CopyIndex,
) -> Result<()> {
    match routes.publish(&backends_of(copies)) {
        Ok(()) => Ok(()),
        Err(e) => Err(e.context(format!(
            "router not confirmed for copy {fresh}; copy {fresh} left running, the next deploy replaces it"
        ))),
    }
}

/// Adds a healthy copy to `copies` and routes to it.
pub(super) fn add_copy(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    copies: &mut Vec<LiveCopy>,
) -> Result<()> {
    let fresh = match start_healthy(runtime, copies) {
        Ok(fresh) => fresh,
        Err(e) => return Err(e),
    };
    let index = fresh.copy;
    copies.push(fresh);
    publish_added(routes, copies, index)
}

/// Removes `old` from the routes, then stops it.
pub(super) fn drain_copy(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    copies: &mut Vec<LiveCopy>,
    old: &LiveCopy,
) -> Result<()> {
    copies.retain(|c| c.copy != old.copy);
    if let Err(e) = routes.publish(&backends_of(copies)) {
        return Err(e.context(format!("could not drain copy {}, left running", old.copy)));
    }
    match runtime.stop(old) {
        Ok(()) => Ok(()),
        Err(e) => Err(e.context(format!("copy {} was drained but did not stop", old.copy))),
    }
}

/// Brings a service to `want` copies without restarting the ones it keeps.
/// Growing starts copies at the lowest free indexes; shrinking drains and
/// stops the highest indexes first.
pub fn scale_to(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    want: u32,
) -> Result<()> {
    let mut copies = match runtime.live() {
        Ok(copies) => copies,
        Err(e) => return Err(e.context("could not list running copies")),
    };
    while copies.len() < want as usize {
        if let Err(e) = add_copy(runtime, routes, &mut copies) {
            return Err(e);
        }
    }
    copies.sort_by_key(|c| std::cmp::Reverse(c.copy.get()));
    let surplus = copies.len() - want as usize;
    let victims: Vec<LiveCopy> = copies.iter().take(surplus).cloned().collect();
    for extra in &victims {
        if let Err(e) = drain_copy(runtime, routes, &mut copies, extra) {
            return Err(e);
        }
    }
    Ok(())
}
