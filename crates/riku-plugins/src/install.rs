//! Plugin install service (ROADMAP E2 / E2.5).
//!
//! Installs a manifest-based plugin bundle from a local path or a git URL into
//! `~/.riku/plugins/`, **verifying its checksum** against the manifest before
//! trusting it, and recording the result in the lockfile. Security: a manifest
//! that pins a `checksum` is rejected on mismatch (the bundle's entry executable
//! is what runs on the host); a bundle with no pinned checksum installs but is
//! flagged unverified.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::config::RikuPaths;
use crate::util::copy_dir_recursive;

use super::bundles;
use super::lockfile::{LockEntry, Lockfile};
use super::manifest::PluginManifest;

/// `sha256:<hex>` digest of a file.
pub fn checksum_of(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("hashing {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn checksum_matches(expected: &str, actual: &str) -> bool {
    let norm = |s: &str| s.trim().trim_start_matches("sha256:").to_ascii_lowercase();
    crate::util::secure::constant_time_eq(&norm(expected), &norm(actual))
}

/// Installs and removes plugin bundles.
pub struct PluginInstaller<'a> {
    paths: &'a RikuPaths,
}

impl<'a> PluginInstaller<'a> {
    pub fn new(paths: &'a RikuPaths) -> Self {
        Self { paths }
    }

    /// Install from `source` (a local directory or a git URL). Returns the
    /// installed manifest.
    pub fn install(&self, source: &str) -> Result<PluginManifest> {
        self.install_with_ref(source, None)
    }

    /// Install, optionally pinning a git `ref` (tag/branch) for git sources.
    pub fn install_with_ref(&self, source: &str, git_ref: Option<&str>) -> Result<PluginManifest> {
        let local = Path::new(source);
        if local.is_dir() {
            if git_ref.is_some() {
                bail!("version pinning is only supported for git sources, not local paths");
            }
            return self.install_from_dir(local, source);
        }
        if let Some(url) = git_url(source) {
            return self.install_from_git(&url, source, git_ref);
        }
        bail!("source '{source}' is not a local directory or a git URL (try ./path or https://…/repo.git)");
    }

    fn install_from_git(
        &self,
        url: &str,
        source: &str,
        git_ref: Option<&str>,
    ) -> Result<PluginManifest> {
        std::fs::create_dir_all(self.paths.cache_root.as_path())?;
        let tmp = self
            .paths
            .cache_root
            .join(format!("plugin-install-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);

        let cloned = (|| {
            let mut cmd = Command::new("git");
            cmd.args(["clone", "--depth", "1", "--quiet"]);
            if let Some(reference) = git_ref {
                cmd.args(["--branch", reference]);
            }
            let status = cmd
                .arg(url)
                .arg(&tmp)
                .status()
                .context("running git clone")?;
            if !status.success() {
                bail!(
                    "git clone of '{url}'{} failed",
                    git_ref.map(|r| format!(" at '{r}'")).unwrap_or_default()
                );
            }
            self.install_from_dir(&tmp, source)
        })();

        let _ = std::fs::remove_dir_all(&tmp);
        cloned
    }

    fn install_from_dir(&self, bundle: &Path, source: &str) -> Result<PluginManifest> {
        let manifest = PluginManifest::from_dir(bundle)?;

        let entry = manifest.entry_path(bundle);
        if !entry.is_file() {
            bail!("manifest entry '{}' not found in bundle", manifest.entry);
        }

        // Trust gate: reject on a pinned-checksum mismatch.
        let actual = checksum_of(&entry)?;
        if let Some(expected) = &manifest.checksum {
            if !checksum_matches(expected, &actual) {
                bail!(
                    "checksum mismatch for '{}': manifest pins {expected}, computed {actual}",
                    manifest.name
                );
            }
        }

        // Signature gate: a signed bundle must verify against a *trusted* key,
        // otherwise it is rejected (not merely flagged).
        let signer = match &manifest.signature {
            Some(signature) => {
                let bytes = std::fs::read(&entry)?;
                match crate::signing::Keyring::new(self.paths)
                    .verifier_of(&bytes, signature)
                {
                    Some(key) => Some(key.name),
                    None => bail!(
                        "plugin '{}' is signed but no trusted key verifies it — add the publisher's key with `riku plugins trust add <name> <pubkey>`",
                        manifest.name
                    ),
                }
            }
            None => None,
        };

        let dest = self.paths.plugin_root.join(&manifest.name);
        if dest.exists() {
            bail!(
                "plugin '{}' is already installed — `riku plugins remove {}` first",
                manifest.name,
                manifest.name
            );
        }
        std::fs::create_dir_all(self.paths.plugin_root.as_path())?;
        copy_dir_recursive(bundle, &dest)?;
        make_executable(&manifest.entry_path(&dest))?;

        Lockfile::new(self.paths).upsert(LockEntry {
            name: manifest.name.clone(),
            source: source.to_string(),
            version: manifest.version.clone(),
            checksum: Some(actual),
            author_pinned: manifest.checksum.is_some(),
            signer,
        })?;

        Ok(manifest)
    }

    /// Remove an installed plugin and its lock entry.
    pub fn remove(&self, name: &str) -> Result<()> {
        if name.contains('/') || name.contains("..") {
            bail!("invalid plugin name '{name}'");
        }
        let dest = self.paths.plugin_root.join(name);
        if !dest.exists() {
            bail!("plugin '{name}' is not installed");
        }
        std::fs::remove_dir_all(&dest)?;
        Lockfile::new(self.paths).remove(name)?;
        Ok(())
    }

    /// Installed bundles paired with their lock entry (if recorded).
    pub fn list(&self) -> Vec<(PluginManifest, Option<LockEntry>)> {
        let locks = Lockfile::new(self.paths).entries();
        bundles::find_bundles(&self.paths.plugin_root)
            .into_iter()
            .map(|(_, manifest)| {
                let lock = locks.iter().find(|l| l.name == manifest.name).cloned();
                (manifest, lock)
            })
            .collect()
    }

    /// Audit every installed bundle: manifest validity, entry presence, and
    /// **integrity** — the entry is re-hashed and compared to the lockfile, so
    /// tampering since install is caught.
    pub fn audit(&self) -> Vec<PluginHealth> {
        let locks = Lockfile::new(self.paths).entries();
        let mut out = Vec::new();

        let Ok(read_dir) = std::fs::read_dir(&self.paths.plugin_root) else {
            return out;
        };
        for entry in read_dir.flatten() {
            let dir = entry.path();
            // Only manifest-based bundles; legacy single-file runtimes are skipped.
            if !dir.is_dir() || !dir.join("riku-plugin.toml").exists() {
                continue;
            }
            let label = dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            out.push(self.audit_bundle(&dir, label, &locks));
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    fn audit_bundle(&self, dir: &Path, label: String, locks: &[LockEntry]) -> PluginHealth {
        let manifest = match PluginManifest::from_dir(dir) {
            Ok(m) => m,
            Err(e) => return PluginHealth::fail(label, format!("invalid manifest: {e}")),
        };

        let entry = manifest.entry_path(dir);
        if !entry.is_file() {
            return PluginHealth::fail(
                manifest.name,
                format!("entry '{}' missing", manifest.entry),
            );
        }

        match locks
            .iter()
            .find(|l| l.name == manifest.name)
            .and_then(|l| l.checksum.as_ref())
        {
            Some(expected) => match checksum_of(&entry) {
                Ok(actual) if checksum_matches(expected, &actual) => PluginHealth::ok(
                    manifest.name,
                    format!("api {} · integrity verified", manifest.api),
                ),
                Ok(_) => PluginHealth::fail(
                    manifest.name,
                    "entry changed since install (checksum mismatch)".into(),
                ),
                Err(e) => PluginHealth::warn(manifest.name, format!("could not hash entry: {e}")),
            },
            None => PluginHealth::warn(
                manifest.name,
                "installed but not in the lockfile (unmanaged — reinstall via `riku plugins`)"
                    .into(),
            ),
        }
    }
}

/// Outcome of auditing one installed plugin.
pub struct PluginHealth {
    pub name: String,
    pub status: HealthStatus,
    pub detail: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum HealthStatus {
    Ok,
    Warn,
    Fail,
}

impl PluginHealth {
    fn ok(name: impl Into<String>, detail: String) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Ok,
            detail,
        }
    }
    fn warn(name: impl Into<String>, detail: String) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Warn,
            detail,
        }
    }
    fn fail(name: impl Into<String>, detail: String) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Fail,
            detail,
        }
    }
}

/// Resolve a git source string to a clone URL. Accepts `github:owner/repo`,
/// `https://…`, `git@…`, and `…/repo.git`. Returns `None` for non-git sources.
pub(crate) fn git_url(source: &str) -> Option<String> {
    if let Some(rest) = source.strip_prefix("github:") {
        return Some(format!("https://github.com/{rest}.git"));
    }
    if source.starts_with("git@")
        || source.ends_with(".git")
        || source.starts_with("https://")
        || source.starts_with("http://")
    {
        return Some(source.to_string());
    }
    None
}

fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn setup() -> (tempfile::TempDir, RikuPaths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        (tmp, paths)
    }

    /// Write a bundle dir with a manifest (optionally pinning `checksum`).
    fn write_bundle(dir: &Path, name: &str, checksum: Option<&str>) {
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let entry = dir.join("bin/addon");
        std::fs::write(&entry, "#!/bin/sh\necho '{}'\n").unwrap();
        std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o644)).unwrap();
        let cs = checksum
            .map(|c| format!("checksum = \"{c}\"\n"))
            .unwrap_or_default();
        std::fs::write(
            dir.join("riku-plugin.toml"),
            format!(
                "name=\"{name}\"\nversion=\"1.0.0\"\ntype=\"addon\"\napi={}\nentry=\"bin/addon\"\n{cs}",
                crate::RIKU_PLUGIN_API
            ),
        )
        .unwrap();
    }

    #[test]
    fn installs_from_local_path_and_records_lock() {
        let (tmp, paths) = setup();
        let src = tmp.path().join("src-bundle");
        write_bundle(&src, "demo", None);

        let installer = PluginInstaller::new(&paths);
        let manifest = installer.install(src.to_str().unwrap()).unwrap();
        assert_eq!(manifest.name, "demo");

        // Installed into the plugin root and executable.
        let entry = paths.plugin_root.join("demo/bin/addon");
        assert!(entry.exists());
        assert!(entry.metadata().unwrap().permissions().mode() & 0o111 != 0);

        // Recorded in the lockfile with a computed checksum.
        let locked = Lockfile::new(&paths).entries();
        assert_eq!(locked.len(), 1);
        assert!(locked[0]
            .checksum
            .as_deref()
            .unwrap()
            .starts_with("sha256:"));
    }

    #[test]
    fn rejects_checksum_mismatch() {
        let (tmp, paths) = setup();
        let src = tmp.path().join("bad");
        write_bundle(&src, "bad", Some("sha256:deadbeef"));

        let err = PluginInstaller::new(&paths)
            .install(src.to_str().unwrap())
            .unwrap_err()
            .to_string();
        assert!(err.contains("checksum mismatch"), "got: {err}");
        // Nothing installed on rejection.
        assert!(!paths.plugin_root.join("bad").exists());
    }

    #[test]
    fn accepts_matching_checksum() {
        let (tmp, paths) = setup();
        let src = tmp.path().join("good");
        write_bundle(&src, "good", None);
        // Compute the real digest and re-pin it.
        let real = checksum_of(&src.join("bin/addon")).unwrap();
        write_bundle(&src, "good", Some(&real));

        assert!(PluginInstaller::new(&paths)
            .install(src.to_str().unwrap())
            .is_ok());
    }

    #[test]
    fn refuses_double_install_then_removes() {
        let (tmp, paths) = setup();
        let src = tmp.path().join("dup");
        write_bundle(&src, "dup", None);
        let installer = PluginInstaller::new(&paths);

        installer.install(src.to_str().unwrap()).unwrap();
        assert!(installer.install(src.to_str().unwrap()).is_err());
        assert_eq!(installer.list().len(), 1);

        installer.remove("dup").unwrap();
        assert!(!paths.plugin_root.join("dup").exists());
        assert!(Lockfile::new(&paths).entries().is_empty());
    }

    #[test]
    fn git_url_normalizes_sources() {
        assert_eq!(
            git_url("github:riku-plugins/postgres").unwrap(),
            "https://github.com/riku-plugins/postgres.git"
        );
        assert!(git_url("https://example.com/x.git").is_some());
        assert!(git_url("./local/path").is_none());
    }

    #[test]
    fn audit_verifies_integrity_and_detects_tampering() {
        let (tmp, paths) = setup();
        let src = tmp.path().join("auditme");
        write_bundle(&src, "auditme", None);
        let installer = PluginInstaller::new(&paths);
        installer.install(src.to_str().unwrap()).unwrap();

        // Freshly installed → verified.
        let audit = installer.audit();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].status, HealthStatus::Ok);

        // Tamper with the installed entry → integrity check fails.
        std::fs::write(
            paths.plugin_root.join("auditme/bin/addon"),
            "#!/bin/sh\nevil\n",
        )
        .unwrap();
        let audit = installer.audit();
        assert_eq!(audit[0].status, HealthStatus::Fail);
        assert!(audit[0].detail.contains("changed since install"));
    }

    #[test]
    fn signed_bundle_requires_a_trusted_key() {
        use crate::signing::{Keypair, Keyring};

        let (tmp, paths) = setup();
        let src = tmp.path().join("signed");
        write_bundle(&src, "signed", None);

        // Sign the entry and pin a top-level signature in the manifest.
        let kp = Keypair::generate();
        let sig = kp.sign_hex(&std::fs::read(src.join("bin/addon")).unwrap());
        std::fs::write(
            src.join("riku-plugin.toml"),
            format!(
                "name=\"signed\"\nversion=\"1.0.0\"\ntype=\"addon\"\napi={}\nentry=\"bin/addon\"\nsignature=\"{sig}\"\n",
                crate::RIKU_PLUGIN_API
            ),
        )
        .unwrap();

        let installer = PluginInstaller::new(&paths);

        // Signed but the key is not trusted → rejected, nothing installed.
        let err = installer
            .install(src.to_str().unwrap())
            .unwrap_err()
            .to_string();
        assert!(err.contains("no trusted key"), "got: {err}");
        assert!(!paths.plugin_root.join("signed").exists());

        // Trust the publisher's key → installs and records the signer.
        Keyring::new(&paths).add("acme", &kp.public_hex()).unwrap();
        installer.install(src.to_str().unwrap()).unwrap();
        assert_eq!(
            Lockfile::new(&paths).entries()[0].signer.as_deref(),
            Some("acme")
        );
    }
}
