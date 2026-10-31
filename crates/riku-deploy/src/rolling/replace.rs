//! The rolling replacement algorithm, independent of what a copy is.
//!
//! A [`CopyRuntime`] starts, probes and stops the copies of one service (a
//! container, or a supervised process). A [`RouteSink`] publishes the backends
//! that should receive traffic and returns only once the router confirmed it.
//! Neither is touched here beyond those calls, so the ordering guarantees can
//! be tested with doubles.

use anyhow::{anyhow, Result};
use riku_container::compose::ReplaceOrder;
use riku_container::engine::ContainerId;
use riku_container::types::CopyIndex;
use riku_util::backend::Backend;

/// What a runtime needs to find a copy again when it stops it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyHandle {
    Container(ContainerId),
    Process(String),
}

/// One running copy of a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveCopy {
    pub copy: CopyIndex,
    /// Where the router sends traffic for this copy. `None` for a copy that is
    /// not routed.
    pub backend: Option<Backend>,
    pub handle: CopyHandle,
}

/// The copies of one service, as one runtime manages them.
pub trait CopyRuntime {
    /// Copies running now, in any order.
    fn live(&self) -> Result<Vec<LiveCopy>>;

    /// Starts a copy under this index and returns it once it exists.
    fn start(&mut self, copy: CopyIndex) -> Result<LiveCopy>;

    /// Returns `Ok` once the copy is ready for traffic, an error on timeout.
    fn wait_healthy(&self, copy: &LiveCopy) -> Result<()>;

    /// Stops a copy with the service's own signal and grace period.
    fn stop(&mut self, copy: &LiveCopy) -> Result<()>;

    /// Removes a copy that never became healthy. It cannot fail the deploy, so
    /// an implementation that fails to remove the copy logs the failure.
    fn discard(&mut self, copy: &LiveCopy);
}

/// Where the router learns which backends are live.
pub trait RouteSink {
    /// Replaces the full backend list. Returns `Ok` only after the router
    /// confirmed the change took effect.
    fn publish(&mut self, backends: &[Backend]) -> Result<()>;
}

/// The sink for services that no router fronts.
pub struct NoRoutes;

impl RouteSink for NoRoutes {
    fn publish(&mut self, _backends: &[Backend]) -> Result<()> {
        Ok(())
    }
}

fn backends_of(copies: &[LiveCopy]) -> Vec<Backend> {
    copies.iter().filter_map(|c| c.backend.clone()).collect()
}

fn lowest_free(copies: &[LiveCopy]) -> Result<CopyIndex> {
    let free = (1..).find_map(|n| match CopyIndex::new(n) {
        Some(index) if copies.iter().all(|c| c.copy != index) => Some(index),
        _ => None,
    });
    match free {
        Some(index) => Ok(index),
        None => Err(anyhow!("no free copy index")),
    }
}

fn start_healthy(runtime: &mut impl CopyRuntime, copies: &[LiveCopy]) -> Result<LiveCopy> {
    let index = match lowest_free(copies) {
        Ok(index) => index,
        Err(e) => return Err(e),
    };
    let fresh = match runtime.start(index) {
        Ok(fresh) => fresh,
        Err(e) => return Err(e),
    };
    if let Err(e) = runtime.wait_healthy(&fresh) {
        runtime.discard(&fresh);
        return Err(e.context(format!("copy {index} did not become healthy")));
    }
    Ok(fresh)
}

fn publish_if_overlap(
    routes: &mut impl RouteSink,
    order: ReplaceOrder,
    copies: &[LiveCopy],
) -> Result<()> {
    match order {
        ReplaceOrder::Overlap => routes.publish(&backends_of(copies)),
        ReplaceOrder::StopFirst => Ok(()),
    }
}

fn drain_and_stop(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    order: ReplaceOrder,
    copies: &mut Vec<LiveCopy>,
    old: &LiveCopy,
) -> Result<()> {
    copies.retain(|c| c.copy != old.copy);
    if let Err(e) = publish_if_overlap(routes, order, copies) {
        return Err(e.context(format!("could not drain copy {}, left running", old.copy)));
    }
    runtime.stop(old)
}

fn add_healthy_copy(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    order: ReplaceOrder,
    copies: &mut Vec<LiveCopy>,
) -> Result<()> {
    let fresh = match start_healthy(runtime, copies) {
        Ok(fresh) => fresh,
        Err(e) => return Err(e),
    };
    copies.push(fresh);
    publish_if_overlap(routes, order, copies)
}

fn replace_one(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    order: ReplaceOrder,
    copies: &mut Vec<LiveCopy>,
    old: &LiveCopy,
) -> Result<()> {
    match order {
        ReplaceOrder::Overlap => {
            if let Err(e) = add_healthy_copy(runtime, routes, order, copies) {
                return Err(e);
            }
            drain_and_stop(runtime, routes, order, copies, old)
        }
        ReplaceOrder::StopFirst => {
            copies.retain(|c| c.copy != old.copy);
            if let Err(e) = runtime.stop(old) {
                return Err(e);
            }
            match add_healthy_copy(runtime, routes, order, copies) {
                Ok(()) => Ok(()),
                Err(e) => Err(
                    e.context("the service is down: its old copy stopped and the new one failed")
                ),
            }
        }
    }
}

/// Replaces every running copy with a fresh one, one at a time, then adjusts
/// the number of copies to `want`.
///
/// With [`ReplaceOrder::Overlap`] a new copy is started and proven healthy
/// before any route changes, the old copy leaves the routes before it is
/// stopped, and a failure at any step leaves the old copies serving. If the
/// router cannot confirm that the new copy was added, that copy stays running
/// (the routes may already point at it) and the next deploy replaces it.
/// [`ReplaceOrder::StopFirst`] is for services nothing routes to: the old copy
/// stops before its replacement starts, and routes are never published.
///
/// Old copies beyond `want` are drained and stopped, highest index first,
/// after the kept ones are replaced.
pub fn replace_all(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    order: ReplaceOrder,
    want: u32,
) -> Result<()> {
    let mut old = match runtime.live() {
        Ok(old) => old,
        Err(e) => return Err(e.context("could not list running copies")),
    };
    old.sort_by_key(|c| c.copy.get());
    let keep = (want as usize).min(old.len());
    let surplus = old.split_off(keep);
    let mut copies: Vec<LiveCopy> = old.iter().chain(&surplus).cloned().collect();

    for previous in &old {
        if let Err(e) = replace_one(runtime, routes, order, &mut copies, previous) {
            return Err(e);
        }
    }
    while copies.len() - surplus.len() < want as usize {
        if let Err(e) = add_healthy_copy(runtime, routes, order, &mut copies) {
            return Err(e);
        }
    }
    for extra in surplus.iter().rev() {
        if let Err(e) = drain_and_stop(runtime, routes, order, &mut copies, extra) {
            return Err(e);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "replace_tests.rs"]
mod tests;
