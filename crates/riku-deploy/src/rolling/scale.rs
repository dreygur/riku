//! Adding and removing copies of a service without touching the others.

use anyhow::{anyhow, Result};
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

/// Publishes the backends of `copies`. Nothing is published when none has a
/// backend: routers refuse an empty list, so the old routes stay until the
/// next copy is added.
fn publish_live(routes: &mut impl RouteSink, copies: &[LiveCopy]) -> Result<()> {
    let backends = backends_of(copies);
    if backends.is_empty() {
        return Ok(());
    }
    routes.publish(&backends)
}

/// Routes to `fresh`, already pushed onto `copies`. When the router cannot
/// confirm, the copy stays running: it may already be serving. The next
/// deploy drains it if it is surplus, or replaces it.
pub(super) fn publish_added(
    routes: &mut impl RouteSink,
    copies: &[LiveCopy],
    fresh: CopyIndex,
) -> Result<()> {
    match publish_live(routes, copies) {
        Ok(()) => Ok(()),
        Err(e) => Err(e.context(format!(
            "router not confirmed for copy {fresh}; copy {fresh} left running, the next deploy drains or replaces it"
        ))),
    }
}

/// Adds a healthy copy to `copies` and routes to it. When the router does
/// not confirm, the copy is withdrawn (see [`withdraw`]).
pub(super) fn add_copy(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    copies: &mut Vec<LiveCopy>,
) -> Result<()> {
    let fresh = match start_healthy(runtime, copies) {
        Ok(fresh) => fresh,
        Err(e) => return Err(e),
    };
    let mut with_fresh = copies.clone();
    with_fresh.push(fresh.clone());
    match publish_live(routes, &with_fresh) {
        Ok(()) => {
            copies.push(fresh);
            Ok(())
        }
        Err(e) => Err(withdraw(runtime, routes, copies, fresh, e)),
    }
}

/// After the router did not confirm `fresh`, publishes the routes of
/// `copies` (without it) again. Once that is confirmed `fresh` serves
/// nothing, so it is stopped and the service is back to its count.
/// Otherwise (no other routed copy, a second refusal, or a failed stop) it
/// may be serving, so it stays running and joins `copies`; the next deploy
/// drains or replaces it.
fn withdraw(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    copies: &mut Vec<LiveCopy>,
    fresh: LiveCopy,
    unconfirmed: anyhow::Error,
) -> anyhow::Error {
    let index = fresh.copy;
    let previous = backends_of(copies);
    let restored = !previous.is_empty() && routes.publish(&previous).is_ok();
    if restored && runtime.stop(&fresh).is_ok() {
        return unconfirmed.context(format!(
            "router not confirmed for copy {index}; copy {index} stopped and the previous routes restored"
        ));
    }
    copies.push(fresh);
    unconfirmed.context(format!(
        "router not confirmed for copy {index}; copy {index} left running, the next deploy drains or replaces it"
    ))
}

/// Removes `old` from the routes, then stops it. When `old` is the last
/// routed copy the routes are left alone (they cannot be empty) and the stop
/// is accepted downtime.
pub(super) fn drain_copy(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    copies: &mut Vec<LiveCopy>,
    old: &LiveCopy,
) -> Result<()> {
    copies.retain(|c| c.copy != old.copy);
    if let Err(e) = publish_live(routes, copies) {
        return Err(e.context(format!("could not drain copy {}, left running", old.copy)));
    }
    match runtime.stop(old) {
        Ok(()) => Ok(()),
        Err(e) => Err(e.context(format!("copy {} was drained but did not stop", old.copy))),
    }
}

/// Brings a service to `want` copies without restarting the ones it keeps.
/// Growing starts copies at the lowest free indexes; shrinking drains and
/// stops the highest indexes first. Scaling to 0 is an error: nothing is
/// started or stopped.
pub fn scale_to(
    runtime: &mut impl CopyRuntime,
    routes: &mut impl RouteSink,
    want: u32,
) -> Result<()> {
    if want == 0 {
        return Err(anyhow!("cannot scale a service to 0 copies"));
    }
    if let Err(e) = routes.check() {
        return Err(e.context("not starting a copy: the router config would be refused"));
    }
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

#[cfg(test)]
#[path = "scale_tests.rs"]
mod tests;
