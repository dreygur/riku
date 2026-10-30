//! The staging directory a sync is assembled in.
//!
//! Security model: every write goes through [`Staging`], which first checks that
//! each directory above the target is a real directory inside staging. Files
//! are created with `create_new`, so an existing path or a symlink at the
//! target is never opened. Symlinks from the manifest are created as links and
//! their targets are never resolved or read.
//!
//! Residual risk: the checks are path based (check, then open), so a process
//! running as the same user that can write inside staging or live could swap a
//! directory for a symlink between the check and the write. Staging is private
//! to a locked apply and only this process writes there, so this needs a local
//! attacker with the receiver's own uid, who could edit live directly anyway.

use crate::guard::{ensure_real_dirs, real_file};
use crate::scan::hash_file;
use crate::wire::RelPath;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub(crate) struct Staging {
    root: PathBuf,
}

impl Staging {
    /// Creates the staging directory itself; it must not exist yet.
    pub(crate) fn create(root: PathBuf) -> anyhow::Result<Staging> {
        match fs::create_dir(&root) {
            Ok(()) => Ok(Staging { root }),
            Err(e) => Err(anyhow::anyhow!(
                "cannot create staging directory {}: {e}",
                root.display()
            )),
        }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn make_dir(&self, path: &RelPath) -> anyhow::Result<()> {
        if let Err(e) = ensure_real_dirs(&self.root, path) {
            return Err(e);
        }
        match fs::create_dir(self.root.join(path.to_path())) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(
                "cannot create directory {}: {e}",
                path.as_str()
            )),
        }
    }

    /// Opens a brand new file for writing at `path`.
    pub(crate) fn create_file(&self, path: &RelPath) -> anyhow::Result<fs::File> {
        if let Err(e) = ensure_real_dirs(&self.root, path) {
            return Err(e);
        }
        let opened = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.root.join(path.to_path()));
        match opened {
            Ok(file) => Ok(file),
            Err(e) => Err(anyhow::anyhow!("cannot create file {}: {e}", path.as_str())),
        }
    }

    /// Copies `path` from `live` into staging. The source must be a regular
    /// file reached through real directories only.
    pub(crate) fn copy_from_live(&self, live: &Path, path: &RelPath) -> anyhow::Result<()> {
        let source = match real_file(live, path) {
            Ok(source) => source,
            Err(e) => return Err(e),
        };
        let mut reader = match fs::File::open(&source) {
            Ok(reader) => reader,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "cannot read live file {}: {e}",
                    path.as_str()
                ))
            }
        };
        let mut writer = match self.create_file(path) {
            Ok(writer) => writer,
            Err(e) => return Err(e),
        };
        match io::copy(&mut reader, &mut writer) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("cannot copy {}: {e}", path.as_str())),
        }
    }

    /// Creates a symlink holding `target` as text. The target is not resolved.
    pub(crate) fn symlink(&self, path: &RelPath, target: &str) -> anyhow::Result<()> {
        if let Err(e) = ensure_real_dirs(&self.root, path) {
            return Err(e);
        }
        match std::os::unix::fs::symlink(target, self.root.join(path.to_path())) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(
                "cannot create symlink {}: {e}",
                path.as_str()
            )),
        }
    }

    /// Fails if staging holds any path not in `listed`, so nothing outside the
    /// manifest can reach live.
    pub(crate) fn ensure_only(&self, listed: &HashSet<&str>) -> anyhow::Result<()> {
        audit_dir(&self.root, "", listed)
    }

    /// Checks size and SHA-256 of a staged file against the manifest, then sets
    /// its mode. A malformed `sha256` never equals a computed hex digest.
    pub(crate) fn verify(
        &self,
        path: &RelPath,
        size: u64,
        sha256: &str,
        mode: u32,
    ) -> anyhow::Result<()> {
        let full = match real_file(&self.root, path) {
            Ok(full) => full,
            Err(e) => return Err(e),
        };
        let actual_size = match fs::metadata(&full) {
            Ok(meta) => meta.len(),
            Err(e) => return Err(anyhow::anyhow!("cannot stat {}: {e}", path.as_str())),
        };
        if actual_size != size {
            return Err(anyhow::anyhow!(
                "{} is {actual_size} bytes, manifest says {size}",
                path.as_str()
            ));
        }
        let actual_hash = match hash_file(&full) {
            Ok(hash) => hash,
            Err(e) => return Err(e),
        };
        if actual_hash != sha256 {
            return Err(anyhow::anyhow!("sha256 mismatch for {}", path.as_str()));
        }
        match fs::set_permissions(&full, fs::Permissions::from_mode(mode)) {
            Ok(()) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("cannot set mode of {}: {e}", path.as_str())),
        }
    }
}

fn audit_dir(dir: &Path, prefix: &str, listed: &HashSet<&str>) -> anyhow::Result<()> {
    let items = match fs::read_dir(dir) {
        Ok(items) => items,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "cannot read staging {}: {e}",
                dir.display()
            ))
        }
    };
    for item in items {
        let item = match item {
            Ok(item) => item,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "cannot list staging {}: {e}",
                    dir.display()
                ))
            }
        };
        let name = item.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if !listed.contains(rel.as_str()) {
            return Err(anyhow::anyhow!(
                "staging holds {rel:?}, which the manifest does not list"
            ));
        }
        let is_dir = match item.file_type() {
            Ok(kind) => kind.is_dir(),
            Err(e) => return Err(anyhow::anyhow!("cannot stat staging {rel:?}: {e}")),
        };
        if is_dir {
            if let Err(e) = audit_dir(&item.path(), &rel, listed) {
                return Err(e);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "staging_tests.rs"]
mod tests;
