//! Riku's own file sync, used instead of rsync (which is not assumed present).
//!
//! The sender scans a tree into a [`Manifest`] of path, size and content hash.
//! The receiver answers with a [`TransferPlan`] listing the files it needs and
//! the ones to delete.
//!
//! Security model: the receiving side treats the manifest and the tar stream as
//! hostile. Every path is a [`RelPath`]; files are written only into a fresh
//! staging directory, never through a symlink, never more bytes than the
//! manifest states, and the live directory is replaced only after every file is
//! verified.

pub mod scan;
pub mod wire;

pub use scan::scan;
pub use wire::{Entry, EntryKind, Manifest, RelPath, TransferPlan};
