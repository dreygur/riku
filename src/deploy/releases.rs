//! Per-app release history for `riku rollback` (repository layer).
//!
//! Each successful deploy appends `"<unix_ts> <sha>"` to
//! `~/.riku/releases/<app>.log`. Rollback redeploys a prior SHA — git-native,
//! fitting Riku's existing model where `apps/<app>` is a git working tree.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::config::RikuPaths;

/// One recorded release.
pub struct Release {
    pub ts: u64,
    pub sha: String,
}

/// Repository for the release log.
pub struct ReleaseLog<'a> {
    paths: &'a RikuPaths,
}

impl<'a> ReleaseLog<'a> {
    pub fn new(paths: &'a RikuPaths) -> Self {
        Self { paths }
    }

    fn file(&self, app: &str) -> PathBuf {
        self.paths
            .riku_root
            .join("releases")
            .join(format!("{app}.log"))
    }

    /// Append a deployed SHA. A redeploy of the current SHA is a no-op, so the
    /// history isn't padded with duplicate consecutive entries.
    pub fn record(&self, app: &str, sha: &str) -> Result<()> {
        if self.list(app).last().map(|r| r.sha.as_str()) == Some(sha) {
            return Ok(());
        }
        let path = self.file(app);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("opening {}", path.display()))?;
        writeln!(file, "{ts} {sha}")?;
        Ok(())
    }

    /// History, oldest first (the current release is last).
    pub fn list(&self, app: &str) -> Vec<Release> {
        let Ok(text) = std::fs::read_to_string(self.file(app)) else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|line| {
                let (ts, sha) = line.split_once(' ')?;
                Some(Release {
                    ts: ts.trim().parse().ok()?,
                    sha: sha.trim().to_string(),
                })
            })
            .collect()
    }

    /// The most recent SHA different from the current one — the default
    /// rollback target.
    pub fn previous(&self, app: &str) -> Option<String> {
        let history = self.list(app);
        let current = history.last()?.sha.clone();
        history
            .iter()
            .rev()
            .map(|r| &r.sha)
            .find(|s| **s != current)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> (tempfile::TempDir, RikuPaths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        (tmp, paths)
    }

    #[test]
    fn records_history_and_finds_previous() {
        let (_tmp, paths) = paths();
        let log = ReleaseLog::new(&paths);
        assert!(log.previous("app").is_none());

        log.record("app", "aaa").unwrap();
        log.record("app", "bbb").unwrap();
        log.record("app", "bbb").unwrap(); // duplicate — ignored
        assert_eq!(log.list("app").len(), 2);
        // Current is bbb; previous distinct is aaa.
        assert_eq!(log.previous("app").as_deref(), Some("aaa"));

        log.record("app", "ccc").unwrap();
        assert_eq!(log.previous("app").as_deref(), Some("bbb"));
    }
}
