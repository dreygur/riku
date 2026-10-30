//! The sync conversation over a byte stream: one JSON line each way, then the
//! bundle.
//!
//! The client sends `{"sync":1,"manifest":{...}}`. The server answers
//! `{"need":[...],"delete":[...]}`. The client sends the zstd compressed tar and
//! closes its writing side, which is how the server sees the bundle end. The
//! server applies it and answers `{"ok":true}` or `{"error":"<message>"}`.
//!
//! This file holds the client side, [`send`], and what both sides share. The
//! server side, [`crate::receive`], is in `serve.rs`.
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

use crate::bundle::write_bundle;
use crate::line::read_line;
use crate::scan::scan;
use crate::wire::{EntryKind, Manifest, RelPath, TransferPlan};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{self, BufRead, Write};
use std::path::Path;

/// The only protocol version this crate speaks.
pub const SYNC_PROTOCOL: u32 = 1;

/// Longest line either side will read, not counting the newline.
pub const MAX_LINE_BYTES: usize = 64 * 1024 * 1024;

const ERROR_TEXT_CHARS: usize = 300;

/// What `send` did, for the caller to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendReport {
    /// Files the server asked for and received, in manifest order.
    pub sent: Vec<RelPath>,
    /// Paths the server removed because the manifest no longer lists them.
    pub deleted: Vec<RelPath>,
    /// Compressed bundle bytes written to the server.
    pub bytes: u64,
}

#[derive(Serialize)]
struct Request<'a> {
    sync: u32,
    manifest: &'a Manifest,
}

#[derive(Deserialize)]
struct Reply {
    need: Option<Vec<RelPath>>,
    delete: Option<Vec<RelPath>>,
    ok: Option<bool>,
    error: Option<String>,
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

/// Syncs the tree under `src` to the server on the other end of the streams.
///
/// `to_server` is dropped once the bundle is written; the caller's stream must
/// close its writing side when dropped (a pipe or a socket does) because that
/// ends the bundle. Returns the server's error text as the error when it
/// refuses.
pub fn send(
    src: &Path,
    mut from_server: impl BufRead,
    mut to_server: impl Write,
) -> anyhow::Result<SendReport> {
    let manifest = match scan(src) {
        Ok(manifest) => manifest,
        Err(e) => return Err(e),
    };
    let request = Request {
        sync: SYNC_PROTOCOL,
        manifest: &manifest,
    };
    if let Err(e) = write_line(&mut to_server, &request) {
        return Err(e);
    }
    let plan = match read_plan(&mut from_server, &manifest) {
        Ok(plan) => plan,
        Err(e) => return Err(e),
    };
    let bytes = match send_bundle(src, &plan.need, to_server) {
        Ok(bytes) => bytes,
        Err(e) => return Err(e),
    };
    if let Err(e) = read_confirmation(&mut from_server) {
        return Err(e);
    }
    Ok(SendReport {
        sent: plan.need,
        deleted: plan.delete,
        bytes,
    })
}

fn read_reply(from: &mut impl BufRead, what: &str) -> anyhow::Result<Reply> {
    let line = match read_line(from, MAX_LINE_BYTES, what) {
        Ok(line) => line,
        Err(e) => return Err(e),
    };
    let reply: Reply = match serde_json::from_slice(&line) {
        Ok(reply) => reply,
        Err(e) => return Err(anyhow::anyhow!("the {what} is not valid: {e}")),
    };
    if let Some(text) = reply.error {
        let text: String = text
            .chars()
            .filter(|c| !c.is_control())
            .take(ERROR_TEXT_CHARS)
            .collect();
        return Err(anyhow::anyhow!("server refused the sync: {text}"));
    }
    Ok(reply)
}

fn read_plan(from: &mut impl BufRead, manifest: &Manifest) -> anyhow::Result<TransferPlan> {
    let reply = match read_reply(from, "plan reply") {
        Ok(reply) => reply,
        Err(e) => return Err(e),
    };
    let (need, delete) = match (reply.need, reply.delete) {
        (Some(need), Some(delete)) => (need, delete),
        _ => return Err(anyhow::anyhow!("the plan reply lacks need or delete")),
    };
    let files: HashSet<&RelPath> = manifest
        .entries
        .iter()
        .filter(|entry| matches!(entry.kind, EntryKind::File { .. }))
        .map(|entry| &entry.path)
        .collect();
    let mut asked: HashSet<&RelPath> = HashSet::new();
    for path in &need {
        if !files.contains(path) {
            return Err(anyhow::anyhow!(
                "server asked for {:?}, which is not a file in the manifest",
                path.as_str()
            ));
        }
        if !asked.insert(path) {
            return Err(anyhow::anyhow!(
                "server asked for {:?} twice",
                path.as_str()
            ));
        }
    }
    Ok(TransferPlan { need, delete })
}

fn send_bundle(src: &Path, need: &[RelPath], to_server: impl Write) -> anyhow::Result<u64> {
    let mut counted = CountingWriter {
        inner: to_server,
        bytes: 0,
    };
    if let Err(e) = write_bundle(src, need, &mut counted) {
        return Err(e);
    }
    if let Err(e) = counted.flush() {
        return Err(anyhow::anyhow!("cannot flush the bundle: {e}"));
    }
    Ok(counted.bytes)
}

fn read_confirmation(from: &mut impl BufRead) -> anyhow::Result<()> {
    match read_reply(from, "confirmation") {
        Ok(reply) if reply.ok == Some(true) => Ok(()),
        Ok(_) => Err(anyhow::anyhow!("server did not confirm the sync")),
        Err(e) => Err(e),
    }
}

struct CountingWriter<W> {
    inner: W,
    bytes: u64,
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = match self.inner.write(buf) {
            Ok(written) => written,
            Err(e) => return Err(e),
        };
        self.bytes += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
