//! Riku's own file sync, used instead of rsync (which is not assumed present).
//!
//! The sender scans a tree into a [`Manifest`] of path, size and content hash.
//! The receiver answers with a [`TransferPlan`] listing the files it needs and
//! the ones to delete. The sender packs the needed files with [`write_bundle`]
//! and the receiver builds and verifies the new tree with [`apply`].
//!
//! Security model: the receiving side treats the manifest and the tar stream as
//! hostile. Every path is a [`RelPath`]; files are written only into a fresh
//! staging directory, never through a symlink, never more bytes than the
//! manifest states, and the live directory is replaced only after every file is
//! verified.

#![allow(clippy::question_mark)]

pub mod apply;
pub mod bundle;
pub mod limits;
pub mod plan;
pub mod scan;
pub mod session;
pub mod wire;

mod capped;
mod check;
mod guard;
mod line;
mod lock;
mod receive;
mod send;
mod serve;
mod staging;
mod swap;

#[cfg(test)]
mod session_kit;
#[cfg(test)]
mod testkit;

pub use apply::apply;
pub use bundle::write_bundle;
pub use limits::{ReceiveLimits, DEFAULT_FREE_MARGIN_BYTES, DEFAULT_MAX_NEEDED_BYTES};
pub use plan::plan_transfer;
pub use scan::scan;
pub use send::send;
pub use serve::receive;
pub use session::{SendReport, MAX_LINE_BYTES, SYNC_PROTOCOL};
pub use wire::{Entry, EntryKind, Manifest, RelPath, TransferPlan};
