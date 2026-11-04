//! The thread that copies container logs, so the supervisor's main loop
//! never waits on a `logs` call.
//!
//! Each watch tick sends the thread the containers it knows. The thread
//! pulls every [`PULL_INTERVAL`] and after each update, using the newest
//! update it has. Each call is bounded by the engine call limit, so a wedged
//! engine only delays log copying. The thread ends once the watch, and with
//! it the sender, is dropped.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use riku_container::engine::Engine;

use super::listing::Known;
use super::log_pull::{LogPuller, Start};

const PULL_INTERVAL: Duration = Duration::from_secs(5);

/// The sending side of the log thread.
pub(crate) struct LogThread {
    tx: Sender<Known>,
}

impl LogThread {
    /// Start the thread. When it cannot be spawned, logs are not copied and
    /// a warning says so; supervision goes on.
    pub(crate) fn spawn(engine: Engine, log_root: PathBuf) -> LogThread {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("riku-container-logs".to_string())
            .spawn(move || run(&engine, LogPuller::new(log_root), &rx));
        if let Err(e) = spawned {
            tracing::warn!("cannot start the container log thread, logs are not copied: {e}");
        }
        LogThread { tx }
    }

    /// Hand the thread the containers the watch knows now. Never blocks.
    pub(crate) fn update(&self, known: &Known) {
        let _ = self.tx.send(known.clone());
    }
}

/// The first round starts adopted copies after their newest line; later
/// rounds take new copies from their first line.
fn run(engine: &Engine, mut puller: LogPuller, rx: &Receiver<Known>) {
    let mut known: Option<Known> = None;
    let mut start = Start::FromNow;
    loop {
        match rx.recv_timeout(PULL_INTERVAL) {
            Ok(update) => known = Some(update),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        while let Ok(update) = rx.try_recv() {
            known = Some(update);
        }
        if let Some(known) = &known {
            puller.tick(engine, known, start);
            start = Start::All;
        }
    }
}

#[cfg(test)]
#[path = "log_thread_tests.rs"]
mod tests;
