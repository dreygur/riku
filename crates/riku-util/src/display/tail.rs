//! The last lines of a streamed command (a build, a release command). A
//! failed deploy does not show them again, since each was streamed already.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// How many lines a tail keeps by default.
pub const FAILURE_TAIL_LINES: usize = 15;

/// A shared ring of the most recent output lines. Clones share the ring, so
/// the threads reading a child's stdout and stderr can both feed it.
#[derive(Clone, Debug)]
pub struct OutputTail {
    lines: Arc<Mutex<VecDeque<String>>>,
    cap: usize,
}

impl Default for OutputTail {
    fn default() -> Self {
        Self::new(FAILURE_TAIL_LINES)
    }
}

impl OutputTail {
    /// A tail that keeps the last `cap` lines.
    pub fn new(cap: usize) -> Self {
        Self {
            lines: Arc::new(Mutex::new(VecDeque::with_capacity(cap))),
            cap,
        }
    }

    /// Keep `line`, dropping the oldest one when full.
    pub fn push(&self, line: &str) {
        let mut lines = match self.lines.lock() {
            Ok(lines) => lines,
            Err(poisoned) => poisoned.into_inner(),
        };
        if lines.len() == self.cap {
            lines.pop_front();
        }
        lines.push_back(line.to_string());
    }

    /// The kept lines, oldest first.
    pub fn lines(&self) -> Vec<String> {
        match self.lines.lock() {
            Ok(lines) => lines.iter().cloned().collect(),
            Err(poisoned) => poisoned.into_inner().iter().cloned().collect(),
        }
    }
}
