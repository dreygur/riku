//! Copying a directory of the app checkout into a published static site.
//!
//! Security: every path, the top level source and each symlink met on the
//! way, is canonicalized and refused unless it stays inside the canonical
//! checkout, since an app repo chooses both the source paths and the links
//! in them. Copies are `2750` (directories) and `0640` (files), so only the
//! static root's group reads them.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

const DIR_MODE: u32 = 0o2750;
const FILE_MODE: u32 = 0o640;
/// Deep enough for any real asset tree, and it ends a symlink loop.
const MAX_DEPTH: usize = 64;

/// Copy the `source` directory of `checkout` into `dest`. A missing source
/// publishes an empty directory, so nginx answers 404 as it did before
/// anything was built there.
#[allow(clippy::question_mark)]
pub(crate) fn copy_source(checkout: &Path, source: &str, dest: &Path) -> Result<()> {
    let path = checkout.join(source);
    if let Err(e) = path.symlink_metadata() {
        if e.kind() == ErrorKind::NotFound {
            crate::util::display::warn(&format!(
                "Static directory '{source}' not found, serving it empty"
            ));
            return make_dir(dest);
        }
        return Err(anyhow!("cannot inspect {}: {e}", path.display()));
    }
    let resolved = match resolve_inside(&path, checkout) {
        Ok(resolved) => resolved,
        Err(e) => return Err(e),
    };
    if !resolved.is_dir() {
        return Err(anyhow!("static source '{source}' is not a directory"));
    }
    copy_tree(&resolved, dest, checkout, 0)
}

/// Create `dir` with the published directory mode.
pub(crate) fn make_dir(dir: &Path) -> Result<()> {
    if let Err(e) = fs::create_dir(dir) {
        return Err(anyhow!("cannot create {}: {e}", dir.display()));
    }
    set_mode(dir, DIR_MODE)
}

/// `path` canonicalized, refused unless it stays inside `checkout`.
fn resolve_inside(path: &Path, checkout: &Path) -> Result<PathBuf> {
    let resolved = match fs::canonicalize(path) {
        Ok(resolved) => resolved,
        Err(e) => return Err(anyhow!("cannot resolve {}: {e}", path.display())),
    };
    if !resolved.starts_with(checkout) {
        return Err(anyhow!(
            "refusing to publish {}: it resolves to {}, outside the app checkout",
            path.display(),
            resolved.display()
        ));
    }
    Ok(resolved)
}

#[allow(clippy::question_mark)]
fn copy_tree(source: &Path, dest: &Path, checkout: &Path, depth: usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(anyhow!(
            "refusing to publish {}: nested deeper than {MAX_DEPTH} directories (a symlink loop?)",
            source.display()
        ));
    }
    if let Err(e) = make_dir(dest) {
        return Err(e);
    }
    let entries = match fs::read_dir(source) {
        Ok(entries) => entries,
        Err(e) => return Err(anyhow!("cannot read {}: {e}", source.display())),
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => return Err(anyhow!("cannot read {}: {e}", source.display())),
        };
        if let Err(e) = copy_entry(
            &entry.path(),
            &dest.join(entry.file_name()),
            checkout,
            depth,
        ) {
            return Err(e);
        }
    }
    Ok(())
}

/// Copy one directory entry. A symlink is followed only into the checkout;
/// sockets, fifos and devices are skipped.
#[allow(clippy::question_mark)]
fn copy_entry(path: &Path, dest: &Path, checkout: &Path, depth: usize) -> Result<()> {
    let source = match resolve_inside(path, checkout) {
        Ok(source) => source,
        Err(e) => return Err(e),
    };
    let meta = match fs::metadata(&source) {
        Ok(meta) => meta,
        Err(e) => return Err(anyhow!("cannot inspect {}: {e}", source.display())),
    };
    if meta.is_dir() {
        return copy_tree(&source, dest, checkout, depth + 1);
    }
    if !meta.is_file() {
        return Ok(());
    }
    if let Err(e) = fs::copy(&source, dest) {
        return Err(anyhow!("cannot copy {}: {e}", source.display()));
    }
    set_mode(dest, FILE_MODE)
}

fn set_mode(path: &Path, mode: u32) -> Result<()> {
    match fs::set_permissions(path, fs::Permissions::from_mode(mode)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot chmod {}: {e}", path.display())),
    }
}
