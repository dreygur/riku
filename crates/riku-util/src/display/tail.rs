//! The last lines of a streamed command (a build, a release command). A
//! failed deploy does not show them again, since each was streamed already.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::sync::{Arc, Mutex};

use crate::deploy_logger::DeployLogSink;

/// How many lines a tail keeps by default.
pub const FAILURE_TAIL_LINES: usize = 15;

/// Longest line handed on whole; a longer run without a newline is split, so
/// one endless line cannot grow a buffer without bound.
const MAX_LINE_BYTES: usize = 64 * 1024;

/// Calls `on_line` for every line `source` writes until end of file, without
/// the line ending. Bytes that are not UTF-8 are replaced rather than ending
/// the read: a reader that stopped early would close the pipe and kill the
/// writer with SIGPIPE.
pub fn for_each_line(source: impl Read, mut on_line: impl FnMut(&str)) {
    let mut reader = BufReader::new(source.take(u64::MAX));
    let mut line = Vec::new();
    loop {
        line.clear();
        let limit = (MAX_LINE_BYTES + 1) as u64;
        reader.get_mut().set_limit(limit);
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let text = String::from_utf8_lossy(&line);
        on_line(text.trim_end_matches(['\n', '\r']));
    }
}

/// A shared ring of the most recent output lines. Clones share the ring, so
/// the threads reading a child's stdout and stderr can both feed it. A tail
/// made by [`OutputTail::logged_to`] also writes every line to the deploy
/// log.
#[derive(Clone)]
pub struct OutputTail {
    lines: Arc<Mutex<VecDeque<String>>>,
    cap: usize,
    log: Option<DeployLogSink>,
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
            log: None,
        }
    }

    /// This tail, also writing every line pushed to it into `log`.
    pub fn logged_to(self, log: DeployLogSink) -> Self {
        Self {
            log: Some(log),
            ..self
        }
    }

    /// Keep `line`, dropping the oldest one when full.
    pub fn push(&self, line: &str) {
        if let Some(log) = &self.log {
            log.write_streamed(line);
        }
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
