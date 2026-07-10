//! Plugin install service (ROADMAP E2 / E2.5).
//!
//! Installs a manifest-based plugin bundle from a local path or a git URL into
//! `~/.riku/plugins/`, **verifying its checksum** against the manifest before
//! trusting it, and recording the result in the lockfile. Security: a manifest
//! that pins a `checksum` is rejected on mismatch (the bundle's entry executable
//! is what runs on the host); a bundle with no pinned checksum installs but is
//! flagged unverified.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::config::RikuPaths;
use crate::executor::{plugin_timeout, spawn_retrying_etxtbsy, wait_with_timeout};
use crate::sandbox::{harden, SandboxPaths};
use crate::util::copy_dir_recursive;
use crate::RIKU_PLUGIN_API;

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

        if manifest.lifecycle.install {
            run_lifecycle_verb(self.paths, &dest, &manifest, "on_install");
        }

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

        // Best-effort: an unreadable/invalid manifest at this point just
        // means no cleanup hook to run, never blocks removal.
        if let Ok(manifest) = PluginManifest::from_dir(&dest) {
            if manifest.lifecycle.uninstall {
                run_lifecycle_verb(self.paths, &dest, &manifest, "on_uninstall");
            }
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

/// Invoke a lifecycle verb (`on_install`/`on_uninstall`) on a plugin's own
/// entry executable. **Always best-effort**: a non-zero exit, timeout, or
/// spawn failure is logged as a warning and never propagated — by the time
/// either verb runs, the install/removal itself has already succeeded (or is
/// about to), so a broken hook script must never leave the plugin half
/// installed or block its removal.
fn run_lifecycle_verb(paths: &RikuPaths, bundle: &Path, manifest: &PluginManifest, verb: &str) {
    let mut cmd = Command::new(manifest.entry_path(bundle));
    cmd.arg(verb)
        .current_dir(bundle)
        .env("RIKU_PLUGIN_API", RIKU_PLUGIN_API.to_string())
        .env("RIKU_ROOT", &paths.riku_root)
        .env("RIKU_PLUGIN_NAME", &manifest.name)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Own process group so a timeout can kill the whole tree, same as
        // every other verb dispatch site (addon, router, event bus).
        .process_group(0);
    if let Some(dir) = crate::plugin_data::plugin_data_path(paths, &manifest.name) {
        cmd.env("RIKU_PLUGIN_DATA_PATH", dir);
    }
    harden(&mut cmd, &manifest.capabilities, &SandboxPaths::default());

    let mut child = match spawn_retrying_etxtbsy(&mut cmd) {
        Ok(child) => child,
        Err(e) => {
            tracing::warn!(plugin = %manifest.name, verb, "failed to spawn lifecycle hook: {e}");
            return;
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::json!({ "name": manifest.name, "version": manifest.version });
        if let Ok(line) = serde_json::to_string(&payload) {
            let _ = writeln!(stdin, "{line}");
        }
    }

    let timed_out = wait_with_timeout(&mut child, plugin_timeout());
    if timed_out {
        tracing::warn!(plugin = %manifest.name, verb, "lifecycle hook timed out");
        return;
    }

    match child.wait() {
        Ok(status) if !status.success() => {
            tracing::warn!(
                plugin = %manifest.name,
                verb,
                "lifecycle hook exited with {}",
                status.code().unwrap_or(-1)
            );
        }
        Err(e) => tracing::warn!(plugin = %manifest.name, verb, "wait failed: {e}"),
        _ => {}
    }
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
