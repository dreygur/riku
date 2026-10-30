//! Lexical path resolution for paths named in a compose file.
//!
//! A path is joined to its base, normalised without touching the disk, and
//! must stay under the app directory. Nothing is canonicalised and the target
//! need not exist, because compose allows a bind source that is created later.
//! The consequence is that a symlink committed inside the app that points
//! elsewhere is not detected here. Text that would be split by the engine's
//! `source:target,option` syntax (`:`, `,`, line breaks, NUL) is refused.

use anyhow::{bail, Result};
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
/// `app_path`. `what` names the field for the error message.
pub(super) fn resolve_inside(
    app_path: &Path,
    base: &Path,
    raw: &str,
    what: &str,
) -> Result<PathBuf> {
    if raw.is_empty() || raw.contains([':', ',', '\n', '\r', '\0']) {
        bail!("{what} {raw:?} is empty or contains ':', ',' or a control character");
    }
    let resolved = normalize(&base.join(raw));
    if !resolved.starts_with(normalize(app_path)) {
        bail!("{what} {raw:?} resolves outside the app directory");
    }
    Ok(resolved)
}
