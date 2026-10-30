//! Wire types exchanged between sender and receiver.
//!
//! Security model: the manifest arrives from the peer and is hostile. A
//! [`RelPath`] cannot be built from an absolute path, a `..` component, a
//! backslash or a NUL, and deserializing one runs the same check, so a parsed
//! [`Manifest`] never holds a path that leaves the sync root.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

/// A validated path relative to the sync root, always `/` separated.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(String);

impl TryFrom<&str> for RelPath {
    type Error = anyhow::Error;

    fn try_from(raw: &str) -> anyhow::Result<Self> {
        if raw.is_empty() || raw.starts_with('/') || raw.contains(['\\', '\0']) {
            return Err(anyhow::anyhow!("invalid sync path {raw:?}"));
        }
        if raw.split('/').any(|part| matches!(part, "" | "." | "..")) {
            return Err(anyhow::anyhow!(
                "invalid sync path {raw:?}: empty, . or .. component"
            ));
        }
        Ok(RelPath(raw.to_string()))
    }
}

impl TryFrom<String> for RelPath {
    type Error = anyhow::Error;

    fn try_from(raw: String) -> anyhow::Result<Self> {
        RelPath::try_from(raw.as_str())
    }
}

impl From<RelPath> for String {
    fn from(path: RelPath) -> String {
        path.0
    }
}

impl RelPath {
    /// The path as sent on the wire.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The path as a relative `PathBuf`, safe to join onto a root.
    pub fn to_path(&self) -> PathBuf {
        PathBuf::from(&self.0)
    }

    /// The containing directory, or `None` for a top level entry.
    pub fn parent(&self) -> Option<RelPath> {
        self.0
            .rsplit_once('/')
            .map(|(dir, _)| RelPath(dir.to_string()))
    }
}

/// What a manifest entry is, with the fields that kind needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EntryKind {
    /// A regular file.
    File {
        /// Length in bytes.
        size: u64,
        /// Lowercase hex SHA-256 of the content.
        sha256: String,
        /// Unix permission bits, masked with `0o777`.
        mode: u32,
    },
    /// A symbolic link, recorded and never followed.
    Symlink {
        /// The link text as stored on disk.
        target: String,
    },
    /// A directory.
    Dir,
}

/// One path in a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Location under the sync root.
    pub path: RelPath,
    /// What lives there.
    #[serde(flatten)]
    pub kind: EntryKind,
}

/// Everything under a sync root.
///
/// Parsing enforces what the receiver relies on: paths strictly increasing (so
/// sorted and unique), every entry's parent listed as a directory (so nothing
/// sits under a symlink or an undeclared directory), and file modes within
/// `0o777` (no setuid, setgid or sticky bits).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "RawManifest")]
pub struct Manifest {
    /// Entries in path order; the root itself is not listed.
    pub entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct RawManifest {
    entries: Vec<Entry>,
}

impl TryFrom<RawManifest> for Manifest {
    type Error = anyhow::Error;

    fn try_from(raw: RawManifest) -> anyhow::Result<Self> {
        let mut dirs: HashSet<&str> = HashSet::new();
        let mut previous: Option<&RelPath> = None;
        for entry in &raw.entries {
            if let Some(prev) = previous {
                if entry.path <= *prev {
                    return Err(anyhow::anyhow!(
                        "manifest entry {:?} is duplicated or out of order after {:?}",
                        entry.path.as_str(),
                        prev.as_str()
                    ));
                }
            }
            previous = Some(&entry.path);
            if let Some(parent) = entry.path.parent() {
                if !dirs.contains(parent.as_str()) {
                    return Err(anyhow::anyhow!(
                        "manifest entry {:?} has no listed directory {:?} above it",
                        entry.path.as_str(),
                        parent.as_str()
                    ));
                }
            }
            match &entry.kind {
                EntryKind::Dir => {
                    dirs.insert(entry.path.as_str());
                }
                EntryKind::File { mode, .. } if mode & !0o777 != 0 => {
                    return Err(anyhow::anyhow!(
                        "manifest file {:?} has mode {mode:o}, outside 0o777",
                        entry.path.as_str()
                    ));
                }
                EntryKind::File { .. } | EntryKind::Symlink { .. } => {}
            }
        }
        Ok(Manifest {
            entries: raw.entries,
        })
    }
}

/// The receiver's answer to a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TransferPlan {
    /// Files the sender must put in the tar stream.
    pub need: Vec<RelPath>,
    /// Paths present on the receiver that the manifest no longer lists.
    pub delete: Vec<RelPath>,
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
