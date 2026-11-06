//! Writing and restoring the files of one app's router config.
//!
//! The helper runs with umask 0177, which would turn a requested 0644 into
//! 0600, so every mode here is set on the open file or directory after it is
//! created, never left to the umask. A file is written to a `.new` sibling
//! and renamed over its target, so nginx reads the old file or the new one,
//! never half of one; the `.new` name does not match the `*.conf` include.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use riku_util::write_dir::check_write_dir;

pub(crate) const PUBLIC_FILE: u32 = 0o644;
pub(crate) const PRIVATE_FILE: u32 = 0o600;
pub(crate) const PUBLIC_DIR: u32 = 0o755;
pub(crate) const PRIVATE_DIR: u32 = 0o700;

fn set_mode(dir: &Path, mode: u32) -> Result<()> {
    match fs::set_permissions(dir, fs::Permissions::from_mode(mode)) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("cannot set the mode of {}: {e}", dir.display())),
    }
}

/// Create `dir` and set it to `mode`. Parents that do not exist yet are
/// created `0755`, since the umask would otherwise leave them unreadable to
/// the nginx worker that must walk through them; parents that exist are not
/// changed beyond the check below.
///
/// Before any mode changes, `dir`, its parent and every parent just created
/// must be real directories owned by `owner` that no group or other user can
/// write ([`check_write_dir`]), so a directory another account planted, or a
/// symlink in its place, is refused and nothing it points at is touched.
pub(crate) fn ensure_dir(dir: &Path, mode: u32, owner: u32) -> Result<()> {
    let created: Vec<&Path> = dir.ancestors().take_while(|each| !each.exists()).collect();
    if let Err(e) = fs::create_dir_all(dir) {
        return Err(anyhow!("cannot create {}: {e}", dir.display()));
    }
    let checked = created
        .iter()
        .skip(1)
        .copied()
        .chain(dir.parent())
        .chain([dir]);
    if let Err(e) = checked
        .into_iter()
        .try_for_each(|each| check_write_dir(each, owner))
    {
        return Err(anyhow!("{e}"));
    }
    created
        .iter()
        .skip(1)
        .try_for_each(|parent| set_mode(parent, PUBLIC_DIR))
        .and_then(|()| set_mode(dir, mode))
}

fn staged(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".new");
    path.with_file_name(name)
}

fn create_staged(path: &Path, mode: u32) -> std::io::Result<File> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(PRIVATE_FILE)
        .open(path)
        .and_then(|file| {
            file.set_permissions(fs::Permissions::from_mode(mode))
                .map(|()| file)
        })
}

fn fill(path: &Path, contents: &[u8], mode: u32) -> std::io::Result<()> {
    create_staged(path, mode)
        .and_then(|mut file| file.write_all(contents).and_then(|()| file.sync_all()))
}

/// Write `contents` to `path` with exactly `mode`.
pub(crate) fn write_file(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
    let temp = staged(path);
    if let Err(e) = fill(&temp, contents, mode) {
        let _ = fs::remove_file(&temp);
        return Err(anyhow!("cannot write {}: {e}", temp.display()));
    }
    match fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&temp);
            Err(anyhow!("cannot move {} into place: {e}", path.display()))
        }
    }
}

/// Remove a file; one that is already gone is fine.
pub(crate) fn remove_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow!("cannot remove {}: {e}", path.display())),
    }
}

/// What a file should hold after a change: new content, or nothing.
pub(crate) struct Planned {
    pub path: PathBuf,
    pub mode: u32,
    pub contents: Option<Vec<u8>>,
}

/// The content of the planned files as they were, to put back if the router
/// refuses the change.
pub(crate) struct Snapshot(Vec<(PathBuf, u32, Option<Vec<u8>>)>);

impl Snapshot {
    pub(crate) fn take(planned: &[Planned]) -> Result<Snapshot> {
        let mut before = Vec::new();
        for file in planned {
            let old = match fs::read(&file.path) {
                Ok(bytes) => Some(bytes),
                Err(e) if e.kind() == ErrorKind::NotFound => None,
                Err(e) => return Err(anyhow!("cannot read {}: {e}", file.path.display())),
            };
            before.push((file.path.clone(), file.mode, old));
        }
        Ok(Snapshot(before))
    }

    /// Whether none of the files existed.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.iter().all(|(_, _, old)| old.is_none())
    }

    /// Put every file back. Tries them all and reports every failure.
    pub(crate) fn restore(&self) -> Result<()> {
        let failures: Vec<String> = self
            .0
            .iter()
            .filter_map(|(path, mode, old)| {
                let done = match old {
                    Some(bytes) => write_file(path, bytes, *mode),
                    None => remove_file(path),
                };
                done.err().map(|e| e.to_string())
            })
            .collect();
        if failures.is_empty() {
            return Ok(());
        }
        Err(anyhow!("{}", failures.join("; ")))
    }
}

/// Make every planned file hold what the plan says, in order.
pub(crate) fn install(planned: &[Planned]) -> Result<()> {
    for file in planned {
        let done = match &file.contents {
            Some(bytes) => write_file(&file.path, bytes, file.mode),
            None => remove_file(&file.path),
        };
        if let Err(e) = done {
            return Err(anyhow!("{e}"));
        }
    }
    Ok(())
}
