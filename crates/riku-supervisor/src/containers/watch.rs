//! One watch tick: adoption on the first, then events, the 30 s listing,
//! log copying, restart counts and restarts.

use std::collections::HashSet;
use std::time::Instant;

use riku_container::engine::Engine;

use super::candidates::{collect_reports, dispatch_restarts, find_candidates};
use super::feed::EventStream;
use super::listing::{drain_events, sample_stats, take_listing, Known};
use super::restarts::Restarts;
use super::tally::RestartTally;
use super::{ContainerWatch, Feed, CONTAINER_STATS_INTERVAL};

/// The event stream is an engine child process of about 30 MB, so it runs
/// only while riku containers exist. With none, the listing a deploy requests
/// (see [`ContainerWatch::request_listing`]), or failing that the 30 s
/// listing, finds the first copy and opens the stream then.
fn open_feed(engine: &Engine, known: &Known) -> Feed {
    if known.is_empty() || !engine.has_events() {
        return Feed::Polling;
    }
    match EventStream::open(engine) {
        Ok(stream) => Feed::Events(stream),
        Err(e) => {
            tracing::warn!("cannot follow container events, polling instead: {e}");
            Feed::Polling
        }
    }
}

fn listing_due(last: Option<Instant>, now: Instant) -> bool {
    match last {
        Some(last) => now.saturating_duration_since(last) >= CONTAINER_STATS_INTERVAL,
        None => true,
    }
}

/// Copies being restarted are left out: the restart replaces the id under
/// the same name before its report arrives, and observing that id first
/// would read as a deploy and reset the count.
fn observe_copies(tally: &mut RestartTally, known: &Known, restarts: &Restarts) {
    let mut names: HashSet<String> = HashSet::new();
    for info in known.values() {
        if info.app.is_none() || info.service.is_none() || info.copy.is_none() {
            continue;
        }
        names.insert(info.name.clone());
        if !restarts.is_in_flight(&info.name) {
            tally.observe(&info.name, &info.id);
        }
    }
    names.extend(restarts.names());
    tally.forget_missing(&names);
}

/// Open the event stream when containers appeared, and drop it, which ends
/// the child, when the last one is gone.
fn match_feed_to_containers(watch: &mut ContainerWatch) {
    match (&watch.feed, watch.known.is_empty()) {
        (Feed::Polling, false) => watch.feed = open_feed(&watch.engine, &watch.known),
        (Feed::Events(_), true) => watch.feed = Feed::Polling,
        _ => {}
    }
}

fn adopt(watch: &mut ContainerWatch, now: Instant) {
    if take_listing(&watch.engine, &mut watch.known) {
        watch.last_listing = Some(now);
        sample_stats(&watch.engine, &watch.known, &mut watch.samples);
    }
    match_feed_to_containers(watch);
    watch.logs.update(&watch.known);
    watch.adopted = true;
}

fn refresh(watch: &mut ContainerWatch, now: Instant) {
    drain_events(&watch.engine, &mut watch.feed, &mut watch.known);
    if listing_due(watch.last_listing, now) {
        if take_listing(&watch.engine, &mut watch.known) {
            sample_stats(&watch.engine, &watch.known, &mut watch.samples);
        }
        match_feed_to_containers(watch);
        watch.last_listing = Some(now);
    }
    watch.logs.update(&watch.known);
}

pub(crate) fn reconcile(watch: &mut ContainerWatch, now: Instant) {
    collect_reports(&mut watch.restarts, &mut watch.tally, &mut watch.known, now);
    if watch.adopted {
        refresh(watch, now);
    } else {
        adopt(watch, now);
    }
    observe_copies(&mut watch.tally, &watch.known, &watch.restarts);
    let candidates = find_candidates(&watch.known, &watch.tally, &watch.restarts, now);
    dispatch_restarts(
        &watch.engine,
        &watch.paths,
        &watch.actions,
        &mut watch.restarts,
        &mut watch.known,
        &watch.tally,
        candidates,
    );
}
