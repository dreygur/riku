//! Server side of the sync conversation, see [`crate::session`].
//!
//! Security model: the peer is hostile. The manifest line is length capped
//! before parsing, the version is checked before the manifest is, and the
//! bundle goes through [`crate::apply`]. The peer is sent only the plan and
//! fixed error texts (plus the version number it sent, parsed as an integer);
//! nothing it supplied and nothing from this server's filesystem, such as a
//! path or an OS error, is put into an error line. The full cause is returned
//! to the caller of [`receive`], which owns logging it.
//!
//! The lock on `live` is taken right after the manifest is read and before
//! anything on the server is read or hashed, so a second concurrent session
//! costs the server nothing.

use crate::apply::apply_locked;
use crate::check::wanted_sizes;
use crate::limits::{available_bytes, tree_bytes, ReceiveLimits};
use crate::line::read_line;
use crate::lock::{acquire, SyncBusy};
use crate::plan::plan_transfer;
use crate::session::{write_line, MAX_LINE_BYTES, SYNC_PROTOCOL};
use crate::swap::recover;
use crate::wire::{Manifest, TransferPlan};
use serde::Deserialize;
use std::io::{BufRead, Write};
use std::path::Path;

#[derive(Deserialize)]
struct RequestVersion {
    sync: u64,
}

#[derive(Deserialize)]
struct RequestManifest {
    manifest: Manifest,
}

/// Why the server stopped: `public` is the only text the peer is told, `cause`
/// is the full reason for the server's own log and caller.
struct Refusal {
    public: String,
    cause: anyhow::Error,
}

impl Refusal {
    fn new(public: &str, cause: anyhow::Error) -> Refusal {
        Refusal {
            public: public.to_string(),
            cause,
        }
    }

    fn plain(text: String) -> Refusal {
        Refusal {
            cause: anyhow::anyhow!(text.clone()),
            public: text,
        }
    }
}

/// Serves one sync of `live` from the client on the other end of the streams.
///
/// Reads the manifest, answers with the plan, applies the bundle that follows
/// until the client closes its side, and confirms. A sync that breaks
/// `limits` is refused before the plan is sent. On any failure the peer gets
/// a fixed error line, `live` is as it was before, and the cause is returned.
/// The cause is not logged here; the caller does that.
pub fn receive(
    live: &Path,
    limits: &ReceiveLimits,
    mut from_client: impl BufRead,
    mut to_client: impl Write,
) -> anyhow::Result<()> {
    match converse(live, limits, &mut from_client, &mut to_client) {
        Ok(()) => write_line(&mut to_client, &serde_json::json!({ "ok": true })),
        Err(refusal) => {
            let told = write_line(
                &mut to_client,
                &serde_json::json!({ "error": refusal.public }),
            );
            if let Err(e) = told {
                tracing::debug!("could not tell the peer about the refusal: {e}");
            }
            Err(refusal.cause)
        }
    }
}

fn converse(
    live: &Path,
    limits: &ReceiveLimits,
    from_client: &mut impl BufRead,
    to_client: &mut impl Write,
) -> Result<(), Refusal> {
    let manifest = match read_request(from_client) {
        Ok(manifest) => manifest,
        Err(refusal) => return Err(refusal),
    };
    let lock = match acquire(live) {
        Ok(lock) => lock,
        Err(e) if e.downcast_ref::<SyncBusy>().is_some() => {
            return Err(Refusal::new("another sync of this app is in progress", e))
        }
        Err(e) => return Err(Refusal::new("cannot lock the app for syncing", e)),
    };
    if let Err(e) = recover(live, &lock) {
        return Err(Refusal::new("cannot recover an interrupted sync", e));
    }
    let plan = match plan_transfer(live, &manifest) {
        Ok(plan) => plan,
        Err(e) => return Err(Refusal::new("cannot work out a transfer plan", e)),
    };
    if let Err(refusal) = check_limits(live, limits, &manifest, &plan) {
        return Err(refusal);
    }
    if let Err(e) = write_line(to_client, &plan) {
        return Err(Refusal::new("cannot send the transfer plan", e));
    }
    match apply_locked(live, &lock, &manifest, &plan, from_client) {
        Ok(()) => Ok(()),
        Err(e) => Err(Refusal::new("the bundle was refused", e)),
    }
}

/// Refuses a sync that asks for more than `limits.max_needed_bytes`, or whose
/// staged tree would leave less than `limits.free_margin_bytes` free beside
/// `live`. Only the server's own limit reaches the peer.
fn check_limits(
    live: &Path,
    limits: &ReceiveLimits,
    manifest: &Manifest,
    plan: &TransferPlan,
) -> Result<(), Refusal> {
    let needed = match wanted_sizes(manifest, plan) {
        Ok(wanted) => wanted
            .values()
            .fold(0u64, |total, size| total.saturating_add(*size)),
        Err(e) => return Err(Refusal::new("the manifest is not valid", e)),
    };
    if needed > limits.max_needed_bytes {
        let limit = limits.max_needed_bytes;
        return Err(Refusal::new(
            &format!("the sync is larger than this server's limit of {limit} bytes"),
            anyhow::anyhow!("sync needs {needed} bytes, over the limit of {limit} bytes"),
        ));
    }
    let parent = live.parent().unwrap_or(live);
    let free = match available_bytes(parent) {
        Ok(free) => free,
        Err(e) => return Err(Refusal::new("cannot check free disk space", e)),
    };
    let staged = tree_bytes(manifest);
    if free < staged.saturating_add(limits.free_margin_bytes) {
        return Err(Refusal::new(
            "the server does not have enough free disk space for this sync",
            anyhow::anyhow!(
                "staging {staged} bytes would leave less than the {} byte free space margin ({free} bytes free)",
                limits.free_margin_bytes
            ),
        ));
    }
    Ok(())
}

fn read_request(from_client: &mut impl BufRead) -> Result<Manifest, Refusal> {
    let line = match read_line(from_client, MAX_LINE_BYTES, "manifest line") {
        Ok(line) => line,
        Err(e) => return Err(Refusal::new(&e.fixed_text(), anyhow::Error::new(e))),
    };
    let version: RequestVersion = match serde_json::from_slice(&line) {
        Ok(version) => version,
        Err(e) => {
            return Err(Refusal::new(
                "the first line is not a sync request",
                anyhow::anyhow!("first line is not a sync request: {e}"),
            ))
        }
    };
    if version.sync != u64::from(SYNC_PROTOCOL) {
        return Err(Refusal::plain(format!(
            "sync protocol {} is not supported, this server speaks {SYNC_PROTOCOL}",
            version.sync
        )));
    }
    match serde_json::from_slice::<RequestManifest>(&line) {
        Ok(request) => Ok(request.manifest),
        Err(e) => Err(Refusal::new(
            "the manifest is not valid",
            anyhow::anyhow!("manifest refused: {e}"),
        )),
    }
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;
