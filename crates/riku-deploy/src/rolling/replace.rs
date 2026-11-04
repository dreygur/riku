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

use super::scale::{add_copy, drain_copy, publish_added, start_healthy};

/// What a runtime needs to find a copy again when it stops it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyHandle {
    /// The id of a container the engine created.
    Container(ContainerId),
    /// The supervisor's name for a worker process, such as `web-2`.
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
    ///
    /// For a routed service the copy must come back with `backend: Some`.
    /// Returning `None` for a routed copy is an error the runtime raises
    /// itself; replacement cannot tell it from an unrouted copy.
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
    /// Replaces the full backend list, which always has at least one entry.
    /// Returns `Ok` only after the router confirmed the change took effect.
    fn publish(&mut self, backends: &[Backend]) -> Result<()>;

    /// Fails when a publish is bound to fail whatever the backends, such as
    /// an app snippet nginx refuses. Runs before any copy starts.
    fn check(&mut self) -> Result<()> {
        Ok(())
    }
}

/// The sink for services that no router fronts.
pub struct NoRoutes;

impl RouteSink for NoRoutes {
    fn publish(&mut self, _backends: &[Backend]) -> Result<()> {
        Ok(())
    }
}

fn stop_first_failure(old: &LiveCopy, still_serving: usize) -> String {
    if still_serving == 0 {
        format!(
            "the service is down: copy {} stopped and its replacement failed",
            old.copy
        )
    } else {
        format!(
            "copy {} stopped and its replacement failed; other copies still serving: {still_serving}",
            old.copy
        )
    }
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
            if let Err(e) = add_copy(runtime, routes, copies) {
                return Err(e);
            }
            drain_copy(runtime, routes, copies, old)
        }
        ReplaceOrder::StopFirst => {
            if let Err(e) = drain_copy(runtime, routes, copies, old) {
                return Err(e);
            }
            let fresh = match start_healthy(runtime, copies) {
                Ok(fresh) => fresh,
                Err(e) => return Err(e.context(stop_first_failure(old, copies.len()))),
            };
            let index = fresh.copy;
            copies.push(fresh);
            publish_added(routes, copies, index)
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
/// and the next deploy drains or replaces it. [`ReplaceOrder::StopFirst`] drains and
/// stops the old copy before its replacement starts, so the two never run
/// together; the service is down in between, and when the old copy is the
/// only routed one the drain publish is skipped because routers refuse an
/// empty list. Unrouted services pass
/// [`NoRoutes`] and the publishes do nothing.
///
/// Old copies beyond `want` are drained and stopped, highest index first,
/// before anything starts. That includes a copy an earlier deploy left
/// running when its router step failed, so a run of failed deploys never
/// adds up copies: the next one drains the leftover first, or fails on the
/// same router before it starts another. `want` must be at least 1;
/// stopping a whole service is the caller's job, because it also removes the
/// route.
pub fn replace_all(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    order: ReplaceOrder,
    want: u32,
) -> Result<()> {
    if want == 0 {
        return Err(anyhow!("cannot replace a service with 0 copies"));
    }
    if let Err(e) = routes.check() {
        return Err(e.context("not starting a copy: the router config would be refused"));
    }
    let mut old = match runtime.live() {
        Ok(old) => old,
        Err(e) => return Err(e.context("could not list running copies")),
    };
    old.sort_by_key(|c| c.copy.get());
    let mut copies = old.clone();
    let keep = (want as usize).min(old.len());
    let surplus = old.split_off(keep);

    for extra in surplus.iter().rev() {
        if let Err(e) = drain_copy(runtime, routes, &mut copies, extra) {
            return Err(e);
        }
    }
    for previous in &old {
        if let Err(e) = replace_one(runtime, routes, order, &mut copies, previous) {
            return Err(e);
        }
    }
    while copies.len() < want as usize {
        if let Err(e) = add_copy(runtime, routes, &mut copies) {
            return Err(e);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "replace_tests.rs"]
mod tests;
