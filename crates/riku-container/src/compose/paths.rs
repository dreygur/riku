//! Path resolution for paths named in a compose file.
//!
//! A path is joined to its base, normalised without touching the disk, and
//! must stay under the app directory. Then every component below the app
//! directory that exists is inspected with `symlink_metadata`, and a symlink
//! anywhere on the way is refused, so a committed `data -> /` cannot turn
//! `./data` into a mount of the host root. The first missing component ends the
//! walk, because compose allows a bind source that is created later. Nothing is
//! canonicalised.
//!
//! The checkout can change between parsing and running, so the step that
//! starts containers must repeat this check. Text that would be split by the
//! engine's `source:target,option` syntax (`:`, `,`, line breaks, NUL) is
//! refused.

use anyhow::{bail, Result};
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolve `raw` against `base` and require the result to be inside
/// `app_path` with no symlink on the way. `what` names the field for the
/// error message, such as `bind source`.
pub(super) fn resolve_inside(
    app_path: &Path,
    base: &Path,
    raw: &str,
    what: &str,
) -> Result<PathBuf> {
    if raw.is_empty() || raw.contains([':', ',', '\n', '\r', '\0']) {
        bail!("{what} {raw:?} is empty or contains ':', ',' or a control character");
    }
    let root = normalize(app_path);
    let resolved = normalize(&base.join(raw));
    let below_root = match resolved.strip_prefix(&root) {
        Ok(rest) => rest.to_path_buf(),
        Err(_) => bail!("{what}s must be inside the app directory, got {raw:?}"),
    };
    if let Err(e) = refuse_symlinks(&root, &below_root, what, raw) {
        return Err(e);
    }
    Ok(resolved)
}

fn refuse_symlinks(root: &Path, below_root: &Path, what: &str, raw: &str) -> Result<()> {
    let mut walked = root.to_path_buf();
    for component in below_root.components() {
        walked.push(component.as_os_str());
        match std::fs::symlink_metadata(&walked) {
            Ok(meta) if meta.file_type().is_symlink() => {
                bail!(
                    "{what} {raw:?} passes through the symlink {}; {what}s may not use symlinks",
                    walked.display()
                );
            }
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => bail!("{what} {raw:?}: cannot inspect {}: {e}", walked.display()),
        }
    }
    Ok(())
}
