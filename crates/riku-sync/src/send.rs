//! Client side of the sync conversation, see [`crate::session`].
//!
//! Security model: the server's lines are length capped and its error text is
//! stripped of control characters. The sender packs only files that the
//! manifest it sent lists as regular files, whatever the plan asks for.

use crate::bundle::write_bundle;
use crate::scan::scan;
use crate::session::{
    read_reply, read_reply_raw, server_refusal, write_line, Reply, SendReport, SYNC_PROTOCOL,
};
use crate::wire::{EntryKind, Manifest, RelPath, TransferPlan};
use serde::Serialize;
use std::collections::HashSet;
use std::io::{self, BufRead, Write};
use std::path::Path;

#[derive(Serialize)]
struct Request<'a> {
    sync: u32,
    manifest: &'a Manifest,
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
        Err(write_failure) => return Err(prefer_server_refusal(&mut from_server, write_failure)),
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

/// A server that refuses mid-bundle stops reading, so the sender sees a broken
/// pipe. The server's error line is already waiting and says why.
fn prefer_server_refusal(from: &mut impl BufRead, write_failure: anyhow::Error) -> anyhow::Error {
    match read_reply_raw(from, "refusal") {
        Ok(Reply {
            error: Some(text), ..
        }) => server_refusal(&text),
        _ => write_failure,
    }
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
#[path = "send_tests.rs"]
mod tests;
