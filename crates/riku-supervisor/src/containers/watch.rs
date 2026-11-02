//! One watch tick: adoption on the first, then events, the 30 s listing,
//! log followers, restart counts and restarts.

use std::collections::HashSet;
use std::time::Instant;

use riku_container::engine::Engine;

use super::candidates::{collect_reports, dispatch_restarts, find_candidates};
use super::feed::EventStream;
use super::listing::{drain_events, is_running, running_ids, sample_stats, take_listing, Known};
use super::log_follow::{LogFollowers, Tail};
use super::restarts::Restarts;
use super::tally::RestartTally;
use super::{ContainerWatch, Feed, CONTAINER_STATS_INTERVAL};

fn open_feed(engine: &Engine) -> Feed {
    if !engine.has_events() {
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

fn follow_running(engine: &Engine, followers: &mut LogFollowers, known: &Known, tail: Tail) {
    for info in known.values().filter(|info| is_running(&info.state)) {
        if let Err(e) = followers.follow(engine, info, tail) {
            tracing::warn!(name = %info.name, "cannot follow logs: {e}");
        }
    }
}

fn refollow_exited(engine: &Engine, followers: &mut LogFollowers, known: &Known) {
    for id in followers.reap() {
        let info = match known.get(&id) {
            Some(info) if is_running(&info.state) => info,
            _ => continue,
        };
        if let Err(e) = followers.follow(engine, info, Tail::FromNow) {
            tracing::warn!(name = %info.name, "cannot follow logs again: {e}");
        }
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

fn adopt(watch: &mut ContainerWatch, now: Instant) {
    watch.feed = open_feed(&watch.engine);
    if take_listing(&watch.engine, &mut watch.known) {
        watch.last_listing = Some(now);
        sample_stats(&watch.engine, &watch.known, &mut watch.samples);
    }
    follow_running(
        &watch.engine,
        &mut watch.followers,
        &watch.known,
        Tail::FromNow,
    );
    watch.adopted = true;
}

fn refresh(watch: &mut ContainerWatch, now: Instant) {
    drain_events(&watch.engine, &mut watch.feed, &mut watch.known);
    if listing_due(watch.last_listing, now) {
        if matches!(watch.feed, Feed::Polling) {
            watch.feed = open_feed(&watch.engine);
        }
        if take_listing(&watch.engine, &mut watch.known) {
            sample_stats(&watch.engine, &watch.known, &mut watch.samples);
        }
        watch.last_listing = Some(now);
    }
    refollow_exited(&watch.engine, &mut watch.followers, &watch.known);
    follow_running(&watch.engine, &mut watch.followers, &watch.known, Tail::All);
    watch.followers.retain(&running_ids(&watch.known));
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
        candidates,
    );
}
