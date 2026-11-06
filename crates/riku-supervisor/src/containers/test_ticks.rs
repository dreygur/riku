//! Driving a watch in tests until a background effect shows up.

use std::time::{Duration, Instant};

use super::ContainerWatch;

const WAIT_LIMIT: Duration = Duration::from_secs(5);

/// Tick `watch` at the fixed time `now` until `done` holds, failing after
/// five real seconds with `what` in the message.
pub(crate) fn tick_until(
    watch: &mut ContainerWatch,
    now: Instant,
    done: impl Fn(&ContainerWatch) -> bool,
    what: &str,
) {
    let started = Instant::now();
    while !done(watch) {
        assert!(
            started.elapsed() < WAIT_LIMIT,
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
        watch.tick(now);
    }
}

/// Wait until `done` holds without ticking, for effects of the watch's
/// background threads, such as log copying.
pub(crate) fn wait_until(done: impl Fn() -> bool, what: &str) {
    let started = Instant::now();
    while !done() {
        assert!(
            started.elapsed() < WAIT_LIMIT,
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
