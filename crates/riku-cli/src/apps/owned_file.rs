//! Reading a file in the deploy user's plugin directory without trusting
//! what is on disk.
//!
//! Security: plugins are executables the deploy user runs, so the refresh
//! on `riku init` must only ever read or replace the deploy user's own
//! regular files. Every directory from the plugin root down to the file is
//! checked with `symlink_metadata` and must be a real directory owned by the
//! effective user. The file itself must be a regular file owned by that user;
//! it is opened with `O_NOFOLLOW` (so a link swapped in after the check is
//! refused by the kernel) and `O_NONBLOCK` (so a FIFO cannot hang the open),
//! and checked again on the open handle.

use std::fs::{Metadata, OpenOptions};
use std::io::{ErrorKind, Read};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

use anyhow::{anyhow, Result};
use nix::fcntl::OFlag;

/// What [`read_owned`] found at a path.
#[derive(Debug)]
pub(crate) enum Owned {
    Missing,
    File(Vec<u8>),
    /// Something is there that riku will not touch, and why.
    Refused(String),
}

#[derive(Clone, Copy)]
enum Kind {
    Dir,
    File,
}

/// Why `meta` at `path` is not a `kind` owned by `uid`, if it is not.
fn refusal(path: &Path, meta: &Metadata, kind: Kind, uid: u32) -> Option<String> {
    if meta.file_type().is_symlink() {
        return Some(format!("{} is a symlink", path.display()));
    }
    let right_kind = match kind {
        Kind::Dir => meta.is_dir(),
        Kind::File => meta.is_file(),
    };
    if !right_kind {
        let want = match kind {
            Kind::Dir => "a directory",
            Kind::File => "a regular file",
        };
        return Some(format!("{} is not {want}", path.display()));
    }
    if meta.uid() != uid {
        return Some(format!("{} is owned by another user", path.display()));
    }
    None
}

/// `Ok(None)` when `path` is a `kind` owned by `uid`.
fn inspect(path: &Path, kind: Kind, uid: u32) -> Result<Option<Owned>> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => Ok(refusal(path, &meta, kind, uid).map(Owned::Refused)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Some(Owned::Missing)),
        Err(e) => Err(anyhow!("cannot inspect {}: {e}", path.display())),
    }
}

fn open_and_read(path: &Path, uid: u32) -> Result<Owned> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags((OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK).bits())
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Owned::Missing),
        Err(e) => return Err(anyhow!("cannot open {}: {e}", path.display())),
    };
    let meta = match file.metadata() {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow!("cannot inspect {}: {e}", path.display())),
    };
    if let Some(why) = refusal(path, &meta, Kind::File, uid) {
        return Ok(Owned::Refused(why));
    }
    let mut bytes = Vec::new();
    match file.read_to_end(&mut bytes) {
        Ok(_) => Ok(Owned::File(bytes)),
        Err(e) => Err(anyhow!("cannot read {}: {e}", path.display())),
    }
}

/// Read `root/relative` (`/` separated, no `..`) when it and every directory
/// on the way, `root` included, belong to the effective user.
pub(crate) fn read_owned(root: &Path, relative: &str) -> Result<Owned> {
    let uid = nix::unistd::geteuid().as_raw();
    let mut path = root.to_path_buf();
    let (dirs, name) = match relative.rsplit_once('/') {
        Some((dirs, name)) => (dirs.split('/').collect(), name),
        None => (Vec::new(), relative),
    };
    match inspect(&path, Kind::Dir, uid) {
        Ok(None) => {}
        Ok(Some(found)) => return Ok(found),
        Err(e) => return Err(e),
    }
    for dir in dirs {
        path.push(dir);
        match inspect(&path, Kind::Dir, uid) {
            Ok(None) => {}
            Ok(Some(found)) => return Ok(found),
            Err(e) => return Err(e),
        }
    }
    path.push(name);
    match inspect(&path, Kind::File, uid) {
        Ok(None) => open_and_read(&path, uid),
        Ok(Some(found)) => Ok(found),
        Err(e) => Err(e),
    }
}
