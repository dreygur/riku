//! The staging directory a sync is assembled in.
//!
//! Security model: every write goes through [`Staging`], which first checks that
//! each directory above the target is a real directory inside staging. Files
//! are created with `create_new`, so an existing path or a symlink at the
//! target is never opened. Symlinks from the manifest are created as links and
//! their targets are never resolved or read.

use crate::guard::{ensure_real_dirs, real_file};
use crate::scan::hash_file;
use crate::wire::RelPath;
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
