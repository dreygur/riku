//! Container engine event stream reader.

use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::mpsc;

use anyhow::{anyhow, Result};
use riku_container::engine::{ContainerId, Engine};

/// Result of draining queued container events from the feed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Drained {
    pub(crate) ids: Vec<ContainerId>,
    pub(crate) closed: bool,
}

/// Long-running event stream connected to the engine's event output.
pub(crate) struct EventStream {
    child: Child,
    ids: mpsc::Receiver<ContainerId>,
}

impl EventStream {
    /// Open a new event stream child process and reader thread.
    pub(crate) fn open(engine: &Engine) -> Result<EventStream> {
        let mut command = engine.events_command();
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::null());

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => return Err(anyhow!("cannot spawn engine events: {e}")),
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(anyhow!("cannot capture engine events stdout"));
            }
        };

        let (tx, rx) = mpsc::channel();
        let thread_res = std::thread::Builder::new()
            .name("riku-event-feed".to_string())
            .spawn(move || {
                let reader = BufReader::new(stdout);
                for line_res in reader.lines() {
                    let line = match line_res {
                        Ok(l) => l,
                        Err(_) => break,
                    };
                    if let Some(id) = riku_container::engine::event_container(&line) {
                        if tx.send(id).is_err() {
                            break;
                        }
                    }
                }
            });

        match thread_res {
            Ok(_) => Ok(EventStream { child, ids: rx }),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(anyhow!("cannot spawn event reader thread: {e}"))
            }
        }
    }

    /// Drain all pending container ids received since the last call.
    pub(crate) fn drain(&mut self) -> Drained {
        let mut ids = Vec::new();
        let mut closed = false;
        loop {
            match self.ids.try_recv() {
                Ok(id) => ids.push(id),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    closed = true;
                    break;
                }
            }
        }
        Drained { ids, closed }
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
#[path = "feed_tests.rs"]
mod tests;
