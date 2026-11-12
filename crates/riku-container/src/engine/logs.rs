//! Viewing and collecting container logs.
//!
//! Security model: container ids are [`ContainerId`]s (hex only) and reach
//! the engine as a separate argv element, never through a shell. A `--since`
//! bound is a timestamp the caller formatted itself, not engine output. A
//! container controls its own output, so each stream is read up to a byte
//! cap and a failed call reports only a short, escaped engine line (see
//! `failure_summary`).

use std::ffi::OsString;
use std::process::ExitStatus;

use anyhow::{anyhow, Result};
use riku_util::display::OutputTail;

use super::bounded::capture_capped;
use super::{ContainerId, Engine};

/// Which part of a container's log history [`Engine::timestamped_logs`] returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogRange {
    /// The whole history.
    All,
    /// Lines stamped at or after this RFC 3339 time. Engines include lines
    /// stamped exactly at the bound.
    Since(String),
    /// Only the newest line.
    Newest,
}

/// The output of one bounded `logs --timestamps` call. Every line starts
/// with the engine's RFC 3339 timestamp and a space, and ends with a newline.
#[derive(Debug, Default)]
pub struct TimestampedLogs {
    /// What the container wrote to stdout.
    pub stdout: String,
    /// What the container wrote to stderr.
    pub stderr: String,
    /// True when stdout reached the byte cap. It then holds only the
    /// complete lines read before the cap, and later lines need another call.
    pub stdout_capped: bool,
    /// The same as `stdout_capped`, for stderr.
    pub stderr_capped: bool,
}

/// A `logs` call failed because the container no longer exists, such as a
/// copy a rolling deploy just replaced. Callers find it with
/// `anyhow::Error::is::<ContainerGone>()` and can stop asking for that id.
#[derive(Debug)]
pub struct ContainerGone {
    summary: String,
}

impl std::fmt::Display for ContainerGone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.summary)
    }
}

impl std::error::Error for ContainerGone {}

/// Longest failure detail kept from the engine's stderr.
const DETAIL_CHARS: usize = 200;

/// Whether `line` starts like an RFC 3339 timestamp (`YYYY-MM-DDT`).
fn is_timestamped(line: &str) -> bool {
    let head = line.as_bytes();
    head.len() >= 11
        && head[..11].iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            10 => *b == b'T',
            _ => b.is_ascii_digit(),
        })
}

/// Why a `logs` call failed, without the container's own output.
///
/// With `--timestamps`, the stderr of `docker logs` is mostly what the
/// container wrote to stderr, and every such line starts with a timestamp.
/// Only the exit status and the last line the engine wrote itself are kept,
/// shortened and escaped, so a container cannot fill or forge the
/// supervisor log through a failing call.
fn failure_summary(status: ExitStatus, engine_line: Option<&str>) -> String {
    match engine_line {
        Some(line) => {
            let detail: String = line.chars().take(DETAIL_CHARS).collect();
            format!("container engine 'logs' failed ({status}): {detail:?}")
        }
        None => format!("container engine 'logs' failed ({status})"),
    }
}

/// The error for a failed `logs` call: [`ContainerGone`] when the engine
/// says the container does not exist (Docker: "No such container: ...",
/// Podman: "... no such container"), otherwise a [`failure_summary`].
fn logs_failure(status: ExitStatus, stderr: &[u8]) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(stderr);
    let engine_line = stderr
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty() && !is_timestamped(line));
    let summary = failure_summary(status, engine_line);
    let gone =
        engine_line.is_some_and(|line| line.to_ascii_lowercase().contains("no such container"));
    if gone {
        anyhow::Error::new(ContainerGone { summary })
    } else {
        anyhow!(summary)
    }
}

/// A stream as text. A capped stream ends at its last newline, so the line
/// the byte cap cut is left for the next call.
fn stream_text(bytes: &[u8], capped: bool) -> String {
    let end = if capped {
        bytes
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(0, |at| at + 1)
    } else {
        bytes.len()
    };
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn args_of<const N: usize>(parts: [&str; N]) -> Vec<OsString> {
    parts.into_iter().map(OsString::from).collect()
}

fn range_args(range: &LogRange) -> Vec<OsString> {
    match range {
        LogRange::All => Vec::new(),
        LogRange::Since(at) => vec!["--since".into(), at.into()],
        LogRange::Newest => args_of(["--tail", "1"]),
    }
}

impl Engine {
    /// Put the container's logs, stdout and stderr alike, on the output rail
    /// and keep the last lines in `recent`.
    ///
    /// Bounded by the engine build limit.
    pub fn print_logs(&self, id: &ContainerId, recent: &OutputTail) -> Result<()> {
        self.run_teed(
            &args_of(["logs", id.as_str()]),
            id.as_str(),
            self.settings().build_timeout,
            recent,
        )
    }

    /// Read the part of the container's logs `range` names, with timestamps,
    /// within the call limit and keeping at most `stream_cap` bytes of each
    /// stream. The engine process exits when it is done, so no client stays
    /// running between calls.
    #[allow(clippy::question_mark)]
    pub fn timestamped_logs(
        &self,
        id: &ContainerId,
        range: &LogRange,
        stream_cap: usize,
    ) -> Result<TimestampedLogs> {
        let mut args = args_of(["logs", "--timestamps"]);
        args.extend(range_args(range));
        args.push(id.as_str().into());
        let out = match capture_capped(
            self.program(),
            &args,
            self.settings().call_timeout,
            stream_cap,
        ) {
            Ok(out) => out,
            Err(e) => return Err(e),
        };
        let capped = out.stdout_capped || out.stderr_capped;
        if !out.status.success() && !capped {
            return Err(logs_failure(out.status, &out.stderr));
        }
        if out.truncated {
            return Err(anyhow!(
                "container engine 'logs' output may be incomplete: a helper held the pipe open past the limit"
            ));
        }
        Ok(TimestampedLogs {
            stdout: stream_text(&out.stdout, out.stdout_capped),
            stderr: stream_text(&out.stderr, out.stderr_capped),
            stdout_capped: out.stdout_capped,
            stderr_capped: out.stderr_capped,
        })
    }
}

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;
