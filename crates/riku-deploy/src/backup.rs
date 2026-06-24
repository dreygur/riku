//! App backup & restore (Track A, Phase 2).
//!
//! Bundles an app's durable state — source (`apps/<app>`), env
//! (`envs/<app>`), volumes (`data/<app>`), and git repo (`repos/<app>.git`) —
//! into a `tar.gz`, and restores it.
//!
//! Security: restore extracts an operator-supplied archive, so every member is
//! validated first — no absolute paths, no `..`, and every entry must live
//! under one of *this app's* directories. A crafted archive cannot write
//! elsewhere on disk or touch another app.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::config::RikuPaths;
use crate::util::validate_app_name;

/// Backup/restore for a single app.
pub struct BackupService<'a> {
    paths: &'a RikuPaths,
}

impl<'a> BackupService<'a> {
    pub fn new(paths: &'a RikuPaths) -> Self {
        Self { paths }
    }

    /// The app's components, as paths relative to `riku_root`.
    fn components(&self, app: &str) -> Vec<String> {
        vec![
            format!("apps/{app}"),
            format!("envs/{app}"),
            format!("data/{app}"),
            format!("repos/{app}.git"),
        ]
    }

    /// Create a `tar.gz` of the app's existing components. Returns the path.
    pub fn backup(&self, app: &str, out: Option<&Path>) -> Result<PathBuf> {
        let app = validate_app_name(app)?;
        let root = &self.paths.riku_root;

        let present: Vec<String> = self
            .components(&app)
            .into_iter()
            .filter(|rel| root.join(rel).exists())
            .collect();
        if present.is_empty() {
            bail!("nothing to back up for app '{app}' (no source/env/data/repo found)");
        }

        let out_path = match out {
            Some(p) => p.to_path_buf(),
            None => std::env::current_dir()?.join(format!(
                "{app}-backup-{}.tar.gz",
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            )),
        };

        let mut cmd = Command::new("tar");
        cmd.arg("-czf").arg(&out_path).arg("-C").arg(root);
        for rel in &present {
            cmd.arg(rel);
        }
        if !cmd.status().context("running tar")?.success() {
            bail!("tar failed creating the backup");
        }
        Ok(out_path)
    }

    /// Restore an app from a `tar.gz`, after validating every archive member.
    pub fn restore(&self, app: &str, archive: &Path) -> Result<()> {
        let app = validate_app_name(app)?;
        if !archive.is_file() {
            bail!("backup file '{}' not found", archive.display());
        }

        let listing = Command::new("tar")
            .arg("-tzf")
            .arg(archive)
            .output()
            .context("listing archive")?;
        if !listing.status.success() {
            bail!("could not read archive '{}'", archive.display());
        }
        let entries: Vec<String> = String::from_utf8_lossy(&listing.stdout)
            .lines()
            .map(str::to_string)
            .collect();
        validate_entries(&app, &entries)?;

        std::fs::create_dir_all(self.paths.riku_root.as_path())?;
        let ok = Command::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(&self.paths.riku_root)
            .status()
            .context("running tar")?
            .success();
        if !ok {
            bail!("tar failed extracting the backup");
        }
        Ok(())
    }
}

fn allowed_prefixes(app: &str) -> [String; 4] {
    [
        format!("apps/{app}"),
        format!("envs/{app}"),
        format!("data/{app}"),
        format!("repos/{app}.git"),
    ]
}

/// Reject any archive member that is absolute, contains `..`, or falls outside
/// this app's directories.
fn validate_entries(app: &str, entries: &[String]) -> Result<()> {
    let prefixes = allowed_prefixes(app);
    for raw in entries {
        let entry = raw.trim().trim_end_matches('/');
        if entry.is_empty() {
            continue;
        }
        if entry.starts_with('/') || entry.split('/').any(|c| c == "..") {
            bail!("refusing to restore unsafe path from archive: '{raw}'");
        }
        let under_app = prefixes
            .iter()
            .any(|p| entry == p || entry.starts_with(&format!("{p}/")));
        if !under_app {
            bail!("archive contains a path outside app '{app}': '{raw}'");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_entries_under_the_app() {
        let entries = vec![
            "apps/myapp/".into(),
            "apps/myapp/app.py".into(),
            "envs/myapp/ENV".into(),
            "data/myapp/db.sqlite".into(),
            "repos/myapp.git/HEAD".into(),
        ];
        assert!(validate_entries("myapp", &entries).is_ok());
    }

    #[test]
    fn rejects_traversal_absolute_and_other_apps() {
        assert!(validate_entries("myapp", &["../etc/passwd".into()]).is_err());
        assert!(validate_entries("myapp", &["/etc/passwd".into()]).is_err());
        assert!(validate_entries("myapp", &["apps/myapp/../../etc".into()]).is_err());
        // A different app's data must not ride along.
        assert!(validate_entries("myapp", &["apps/otherapp/x".into()]).is_err());
        // A prefix-collision app name is not a match.
        assert!(validate_entries("myapp", &["apps/myapp-evil/x".into()]).is_err());
    }
}
