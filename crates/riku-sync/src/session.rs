//! The sync conversation over a byte stream: one JSON line each way, then the
//! bundle.
//!
//! The client sends `{"sync":1,"manifest":{...}}`. The server answers
//! `{"need":[...],"delete":[...]}`. The client sends the zstd compressed tar and
//! closes its writing side, which is how the server sees the bundle end. The
//! server applies it and answers `{"ok":true}` or `{"error":"<message>"}`.
//!
//! This file holds the constants and line helpers both sides share. The client
//! is in `send.rs`, the server in `serve.rs`.
//!
//! Security model: the server side talks to a hostile peer, see `serve.rs`.
//! Every line, on both sides, is length capped by [`MAX_LINE_BYTES`] before it
//! is buffered or parsed. The sender packs only files that the manifest it sent
//! lists as regular files, and strips control characters from the server's
//! error text before returning it.
//!
//! There are no timeouts here: a peer that stops sending without closing the
//! stream holds `receive` (and the lock on `live`) until the transport gives
//! up. The code that runs this over ssh must set that limit.

use crate::line::read_line;
use crate::wire::RelPath;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};

/// The only protocol version this crate speaks.
pub const SYNC_PROTOCOL: u32 = 1;

/// Longest line either side will read, not counting the newline.
pub const MAX_LINE_BYTES: usize = 64 * 1024 * 1024;

const ERROR_TEXT_CHARS: usize = 300;

/// What `send` did, for the caller to report.
///
/// `deleted` is the server's own list. A caller that prints its paths must
/// strip control characters first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendReport {
    /// Files the server asked for and received, in manifest order.
    pub sent: Vec<RelPath>,
    /// Paths the server removed because the manifest no longer lists them.
    pub deleted: Vec<RelPath>,
    /// Compressed bundle bytes written to the server.
    pub bytes: u64,
}

/// One line from the server: a plan, a confirmation or an error.
#[derive(Deserialize)]
pub(crate) struct Reply {
    pub(crate) need: Option<Vec<RelPath>>,
    pub(crate) delete: Option<Vec<RelPath>>,
    pub(crate) ok: Option<bool>,
    pub(crate) error: Option<String>,
}

pub(crate) fn write_line(to: &mut impl Write, line: &impl Serialize) -> anyhow::Result<()> {
    let mut bytes = match serde_json::to_vec(line) {
        Ok(bytes) => bytes,
        Err(e) => return Err(anyhow::anyhow!("cannot encode a sync line: {e}")),
    };
    bytes.push(b'\n');
    if let Err(e) = to.write_all(&bytes).and_then(|()| to.flush()) {
        return Err(anyhow::anyhow!("cannot write to the peer: {e}"));
    }
    Ok(())
}

/// Reads and parses one server line. An `error` line is returned as a `Reply`
/// with `error` set, see [`read_reply`] to turn it into an `Err`.
pub(crate) fn read_reply_raw(from: &mut impl BufRead, what: &str) -> anyhow::Result<Reply> {
    let line = match read_line(from, MAX_LINE_BYTES, what) {
        Ok(line) => line,
        Err(e) => return Err(anyhow::Error::new(e)),
    };
    match serde_json::from_slice(&line) {
        Ok(reply) => Ok(reply),
        Err(e) => Err(anyhow::anyhow!("the {what} is not valid: {e}")),
    }
}

pub(crate) fn read_reply(from: &mut impl BufRead, what: &str) -> anyhow::Result<Reply> {
    let reply = match read_reply_raw(from, what) {
        Ok(reply) => reply,
        Err(e) => return Err(e),
    };
    if let Some(text) = &reply.error {
        return Err(server_refusal(text));
    }
    Ok(reply)
}

/// The server's error text as an error, safe to print: control characters are
/// dropped and the text is cut short.
pub(crate) fn server_refusal(text: &str) -> anyhow::Error {
    let text: String = text
        .chars()
        .filter(|c| !c.is_control())
        .take(ERROR_TEXT_CHARS)
        .collect();
    anyhow::anyhow!("server refused the sync: {text}")
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
