//! Copying container output into copy log files with one short `logs` call
//! per container on each round of the log thread (see [`super::log_thread`]),
//! so no engine client stays running between rounds.
//!
//! Each container has a cursor: the newest engine timestamp written and how
//! many lines carry it. Engines include lines stamped exactly at a `--since`
//! bound, so those lines come back on the next call and the cursor skips the
//! ones already written. Lines from stdout and stderr are merged in
//! timestamp order. A line whose timestamp does not parse is dropped.
//!
//! Security model: a container controls its labels and its output. The app,
//! service and copy labels are validated when the listing is parsed, the log
//! path must stay under the log root, and the file is opened with
//! `O_NOFOLLOW`. Each call keeps at most [`STREAM_CAP`] bytes per stream, so
//! a container that floods its output cannot exhaust supervisor memory.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use riku_container::engine::{ContainerId, ContainerInfo, Engine, LogRange, TimestampedLogs};

use super::listing::{is_running, Known};

/// Where a container seen for the first time starts in its log history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Start {
    /// After its newest line, for copies an earlier supervisor already logged.
    FromNow,
    /// From its first line, for copies started since this supervisor began.
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    at: DateTime<Utc>,
    lines_at: usize,
}

struct Stamped<'a> {
    at: DateTime<Utc>,
    text: &'a str,
}

fn stamped(output: &str) -> impl Iterator<Item = Stamped<'_>> {
    output.lines().filter_map(|line| {
        let (at, text) = line.split_once(' ').unwrap_or((line, ""));
        match DateTime::parse_from_rfc3339(at) {
            Ok(at) => Some(Stamped {
                at: at.with_timezone(&Utc),
                text,
            }),
            Err(_) => None,
        }
    })
}

/// The time up to which both streams are complete when one of them hit the
/// byte cap: the newest line of the capped stream, or the older of the two
/// when both were capped. `None` when neither was.
fn cut_off(logs: &TimestampedLogs) -> Option<DateTime<Utc>> {
    [
        (&logs.stdout, logs.stdout_capped),
        (&logs.stderr, logs.stderr_capped),
    ]
    .into_iter()
    .filter(|(_, capped)| *capped)
    .filter_map(|(text, _)| stamped(text).last().map(|line| line.at))
    .min()
}

/// Both streams in timestamp order, without lines past the [`cut_off`] so
/// the next call picks them up. The sort is stable, so lines stamped alike
/// keep the same order on every call.
fn merged(logs: &TimestampedLogs) -> Vec<Stamped<'_>> {
    let mut lines: Vec<Stamped> = stamped(&logs.stdout).chain(stamped(&logs.stderr)).collect();
    lines.sort_by_key(|line| line.at);
    if let Some(cut) = cut_off(logs) {
        lines.retain(|line| line.at <= cut);
    }
    lines
}

/// Whether `path` is `root` followed by plain names only.
fn stays_under(path: &Path, root: &Path) -> bool {
    match path.strip_prefix(root) {
        Ok(rest) => rest
            .components()
            .all(|part| matches!(part, Component::Normal(_))),
        Err(_) => false,
    }
}

fn unseen<'a, 'b>(lines: &'a [Stamped<'b>], cursor: Option<Cursor>) -> &'a [Stamped<'b>] {
    let cursor = match cursor {
        Some(cursor) => cursor,
        None => return lines,
    };
    let older = lines.iter().take_while(|line| line.at < cursor.at).count();
    let repeated = lines[older..]
        .iter()
        .take_while(|line| line.at == cursor.at)
        .count();
    &lines[older + repeated.min(cursor.lines_at)..]
}

#[allow(clippy::question_mark)]
fn cursor_after(lines: &[Stamped]) -> Option<Cursor> {
    let newest = match lines.last() {
        Some(line) => line.at,
        None => return None,
    };
    Some(Cursor {
        at: newest,
        lines_at: lines.iter().filter(|line| line.at == newest).count(),
    })
}

/// Most bytes one `logs` call keeps from each stream.
const STREAM_CAP: usize = 8 * 1024 * 1024;

/// Log cursors of the containers being copied, keyed by id. `None` means
/// nothing was written yet, so the next call asks for the whole history.
pub(crate) struct LogPuller {
    log_root: PathBuf,
    cursors: HashMap<ContainerId, Option<Cursor>>,
    stream_cap: usize,
}

impl LogPuller {
    pub(crate) fn new(log_root: PathBuf) -> Self {
        Self {
            log_root,
            cursors: HashMap::new(),
            stream_cap: STREAM_CAP,
        }
    }

    /// Copy new output of every running container, then give each container
    /// that stopped or went away since the last tick one final call and
    /// forget it.
    pub(crate) fn tick(&mut self, engine: &Engine, known: &Known, start: Start) {
        for info in known.values().filter(|info| is_running(&info.state)) {
            if !self.cursors.contains_key(&info.id) {
                self.start(engine, info, start);
            }
            self.pull(engine, info);
        }
        let stopped: Vec<ContainerId> = self
            .cursors
            .keys()
            .filter(|id| !known.get(*id).is_some_and(|info| is_running(&info.state)))
            .cloned()
            .collect();
        for id in stopped {
            if let Some(info) = known.get(&id) {
                self.pull(engine, info);
            }
            self.cursors.remove(&id);
        }
    }

    /// `{log_root}/{app}/{service}.{copy}.log`, or `None` for a container
    /// missing one of those labels or whose labels lead outside the root.
    fn log_path(&self, info: &ContainerInfo) -> Option<PathBuf> {
        let path = match (&info.app, &info.service, &info.copy) {
            (Some(app), Some(service), Some(copy)) => self
                .log_root
                .join(app)
                .join(format!("{service}.{copy}.log")),
            _ => return None,
        };
        if !stays_under(&path, &self.log_root) {
            tracing::warn!(name = %info.name, "not copying logs: labels lead outside the log directory");
            return None;
        }
        Some(path)
    }

    fn start(&mut self, engine: &Engine, info: &ContainerInfo, start: Start) {
        if self.log_path(info).is_none() {
            return;
        }
        let cursor = match start {
            Start::All => None,
            Start::FromNow => {
                match engine.timestamped_logs(&info.id, &LogRange::Newest, self.stream_cap) {
                    Ok(logs) => cursor_after(&merged(&logs)),
                    Err(e) => {
                        tracing::warn!(name = %info.name, "cannot read the newest log line: {e}");
                        return;
                    }
                }
            }
        };
        self.cursors.insert(info.id.clone(), cursor);
    }

    fn pull(&mut self, engine: &Engine, info: &ContainerInfo) {
        let cursor = match self.cursors.get(&info.id) {
            Some(cursor) => *cursor,
            None => return,
        };
        let range = match cursor {
            Some(cursor) => LogRange::Since(cursor.at.to_rfc3339_opts(SecondsFormat::Nanos, true)),
            None => LogRange::All,
        };
        let logs = match engine.timestamped_logs(&info.id, &range, self.stream_cap) {
            Ok(logs) => logs,
            Err(e) => {
                tracing::warn!(name = %info.name, "cannot read logs: {e}");
                return;
            }
        };
        let lines = merged(&logs);
        let fresh = unseen(&lines, cursor);
        if !fresh.is_empty() {
            if let Err(e) = self.append(info, fresh) {
                tracing::warn!(name = %info.name, "cannot write logs: {e}");
                return;
            }
        }
        if let Some(next) = cursor_after(&lines) {
            self.cursors.insert(info.id.clone(), Some(next));
        }
    }

    fn append(&self, info: &ContainerInfo, lines: &[Stamped]) -> Result<()> {
        let path = match self.log_path(info) {
            Some(path) => path,
            None => return Ok(()),
        };
        if let Some(dir) = path.parent() {
            if let Err(e) = fs::create_dir_all(dir) {
                return Err(anyhow!("cannot create log directory: {e}"));
            }
        }
        let mut file = match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
        {
            Ok(file) => file,
            Err(e) => return Err(anyhow!("cannot open log file: {e}")),
        };
        let text: String = lines
            .iter()
            .map(|line| format!("{}\n", line.text))
            .collect();
        match file.write_all(text.as_bytes()) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow!("cannot append to log file: {e}")),
        }
    }
}

#[cfg(test)]
#[path = "log_pull_tests.rs"]
mod tests;
