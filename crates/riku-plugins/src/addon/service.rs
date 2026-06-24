//! Addon service — orchestrates the addon lifecycle (`PLUGIN_PROTOCOL.md` §6.1).
//!
//! Ties together bundle discovery, verb dispatch, the instance registry, and
//! app env injection. It runs the addon plugin's verbs and keeps the on-disk
//! state consistent; it performs no terminal I/O (the CLI layer does).
//!
//! Env injection is a **non-destructive raw merge**: `bind` rewrites only the
//! keys it owns and leaves every other ENV line byte-for-byte, so it never
//! expands or clobbers unrelated `$VAR` settings; `unbind` removes exactly the
//! keys it recorded.

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};

use crate::config::RikuPaths;
use crate::bundles;
use crate::manifest::PluginManifest;

use super::dispatch::{run_verb, VerbCall};
use super::state::{InstanceRecord, InstanceStore};

/// Business logic for the addon seam.
pub struct AddonService<'a> {
    paths: &'a RikuPaths,
}

impl<'a> AddonService<'a> {
    pub fn new(paths: &'a RikuPaths) -> Self {
        Self { paths }
    }

    fn store(&self) -> InstanceStore<'a> {
        InstanceStore::new(self.paths)
    }

    /// Per-instance data directory handed to the plugin as `RIKU_ADDON_DATA_PATH`.
    fn data_path(&self, plugin: &str, instance: &str) -> PathBuf {
        self.paths
            .data_root
            .join("addons")
            .join(plugin)
            .join(instance)
    }

    fn bundle_for(&self, record: &InstanceRecord) -> Result<(PathBuf, PluginManifest)> {
        bundles::find_addon(&self.paths.plugin_root, &record.plugin).ok_or_else(|| {
            anyhow!(
                "addon plugin '{}' for instance '{}' is not installed",
                record.plugin,
                record.instance
            )
        })
    }

    /// List all provisioned instances.
    pub fn list(&self) -> Vec<InstanceRecord> {
        self.store().list()
    }

    /// Provision a new instance of `plugin` named `instance`.
    pub fn provision(&self, plugin: &str, instance: &str) -> Result<()> {
        let store = self.store();
        if store.exists(instance) {
            bail!("addon instance '{instance}' already exists");
        }
        let (bundle, manifest) = bundles::find_addon(&self.paths.plugin_root, plugin)
            .ok_or_else(|| anyhow!("no addon plugin named '{plugin}' is installed"))?;

        let data_path = self.data_path(plugin, instance);
        std::fs::create_dir_all(&data_path)?;

        run_verb(VerbCall {
            paths: self.paths,
            bundle: &bundle,
            manifest: &manifest,
            verb: "provision",
            instance,
            data_path: &data_path,
            app: None,
            input: serde_json::json!({ "instance": instance }),
        })?;

        store.save(&InstanceRecord::new(plugin, instance))
    }

    /// Bind `app` to `instance`, injecting the env the addon returns.
    /// Returns the injected keys.
    pub fn bind(&self, instance: &str, app: &str) -> Result<Vec<String>> {
        let store = self.store();
        let mut record = store.load(instance)?;
        let (bundle, manifest) = self.bundle_for(&record)?;
        let data_path = self.data_path(&record.plugin, instance);

        let result = run_verb(VerbCall {
            paths: self.paths,
            bundle: &bundle,
            manifest: &manifest,
            verb: "bind",
            instance,
            data_path: &data_path,
            app: Some(app),
            input: serde_json::json!({ "instance": instance, "app": app }),
        })?;

        let vars = env_vars_from(&result);
        let keys = self.set_env_keys(app, &vars)?;
        record.bindings.insert(app.to_string(), keys.clone());
        store.save(&record)?;
        Ok(keys)
    }

    /// Unbind `app` from `instance`, removing the injected env.
    pub fn unbind(&self, instance: &str, app: &str) -> Result<()> {
        let store = self.store();
        let mut record = store.load(instance)?;
        if !record.bindings.contains_key(app) {
            bail!("app '{app}' is not bound to instance '{instance}'");
        }
        let (bundle, manifest) = self.bundle_for(&record)?;
        let data_path = self.data_path(&record.plugin, instance);

        run_verb(VerbCall {
            paths: self.paths,
            bundle: &bundle,
            manifest: &manifest,
            verb: "unbind",
            instance,
            data_path: &data_path,
            app: Some(app),
            input: serde_json::json!({ "instance": instance, "app": app }),
        })?;

        if let Some(keys) = record.bindings.remove(app) {
            self.remove_env_keys(app, &keys)?;
        }
        store.save(&record)
    }

    /// Destroy `instance`. Refuses while apps are still bound (data loss guard).
    pub fn deprovision(&self, instance: &str) -> Result<()> {
        let store = self.store();
        let record = store.load(instance)?;
        if !record.bindings.is_empty() {
            let apps: Vec<&str> = record.bindings.keys().map(String::as_str).collect();
            bail!(
                "instance '{instance}' is still bound to {} — unbind first",
                apps.join(", ")
            );
        }
        let (bundle, manifest) = self.bundle_for(&record)?;
        let data_path = self.data_path(&record.plugin, instance);

        run_verb(VerbCall {
            paths: self.paths,
            bundle: &bundle,
            manifest: &manifest,
            verb: "deprovision",
            instance,
            data_path: &data_path,
            app: None,
            input: serde_json::json!({ "instance": instance }),
        })?;

        if data_path.exists() {
            std::fs::remove_dir_all(&data_path)?;
        }
        store.delete(instance)
    }

    /// Back up `instance`; returns the artifact path the addon reports.
    pub fn backup(&self, instance: &str) -> Result<Option<String>> {
        let store = self.store();
        let record = store.load(instance)?;
        let (bundle, manifest) = self.bundle_for(&record)?;
        let data_path = self.data_path(&record.plugin, instance);

        let result = run_verb(VerbCall {
            paths: self.paths,
            bundle: &bundle,
            manifest: &manifest,
            verb: "backup",
            instance,
            data_path: &data_path,
            app: None,
            input: serde_json::json!({ "instance": instance }),
        })?;
        Ok(result
            .get("artifact")
            .and_then(|v| v.as_str())
            .map(str::to_string))
    }

    fn app_env_file(&self, app: &str) -> PathBuf {
        self.paths.env_root.join(app).join("ENV")
    }

    /// Set `vars` in the app ENV, replacing those keys and preserving all other
    /// lines verbatim. Returns the keys written (sorted).
    fn set_env_keys(&self, app: &str, vars: &[(String, String)]) -> Result<Vec<String>> {
        let path = self.app_env_file(app);
        let owned: BTreeSet<&str> = vars.iter().map(|(k, _)| k.as_str()).collect();

        let mut out = String::new();
        for line in std::fs::read_to_string(&path).unwrap_or_default().lines() {
            if !owned.contains(line_key(line)) {
                out.push_str(line);
                out.push('\n');
            }
        }
        for (k, v) in vars {
            out.push_str(&format!("{k}={v}\n"));
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::util::write_atomic(&path, out.as_bytes())?;

        let mut keys: Vec<String> = vars.iter().map(|(k, _)| k.clone()).collect();
        keys.sort();
        Ok(keys)
    }

    /// Remove `keys` from the app ENV, leaving every other line untouched.
    fn remove_env_keys(&self, app: &str, keys: &[String]) -> Result<()> {
        let path = self.app_env_file(app);
        if !path.exists() {
            return Ok(());
        }
        let remove: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
        let existing = std::fs::read_to_string(&path)?;
        let mut out = String::new();
        for line in existing.lines() {
            if !remove.contains(line_key(line)) {
                out.push_str(line);
                out.push('\n');
            }
        }
        crate::util::write_atomic(&path, out.as_bytes())
    }
}

/// The key of an ENV line (`KEY=VALUE` → `KEY`); non-assignment lines (blanks,
/// comments) yield a key that never matches a real env key, so they survive.
fn line_key(line: &str) -> &str {
    line.split('=').next().unwrap_or("").trim()
}

/// Extract the `{ "env": { ... } }` map an addon's `bind` returns as
/// `(key, value)` string pairs. Non-string values are JSON-encoded.
fn env_vars_from(result: &serde_json::Value) -> Vec<(String, String)> {
    result
        .get("env")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| {
                    let value = v
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| v.to_string());
                    (k.clone(), value)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A fake addon implementing every verb: provision/unbind/deprovision/backup
    /// echo small JSON; bind returns a DATABASE_URL built from the instance.
    const FAKE_ADDON: &str = r#"#!/bin/sh
verb="$1"
cat >/dev/null   # drain the request JSON
case "$verb" in
  bind) printf '{"env":{"DATABASE_URL":"postgres:///%s"}}' "$RIKU_ADDON_INSTANCE" ;;
  backup) echo '{"artifact":"/tmp/db.tar"}' ;;
  *) echo '{}' ;;
esac
"#;

    fn setup() -> (tempfile::TempDir, RikuPaths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        let bundle = paths.plugin_root.join("fakedb");
        std::fs::create_dir_all(bundle.join("bin")).unwrap();
        let entry = bundle.join("bin/addon");
        std::fs::write(&entry, FAKE_ADDON).unwrap();
        std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(
            bundle.join("riku-plugin.toml"),
            format!(
                "name=\"fakedb\"\nversion=\"1\"\ntype=\"addon\"\napi={}\nentry=\"bin/addon\"\n",
                crate::RIKU_PLUGIN_API
            ),
        )
        .unwrap();
        (tmp, paths)
    }

    #[test]
    fn full_lifecycle_provision_bind_unbind_deprovision() {
        let (_tmp, paths) = setup();
        let svc = AddonService::new(&paths);

        svc.provision("fakedb", "db1").unwrap();
        assert!(paths.data_root.join("addons/fakedb/db1").is_dir());
        assert_eq!(svc.list().len(), 1);

        let keys = svc.bind("db1", "myapp").unwrap();
        assert_eq!(keys, vec!["DATABASE_URL".to_string()]);
        let env = std::fs::read_to_string(paths.env_root.join("myapp/ENV")).unwrap();
        assert!(env.contains("DATABASE_URL=postgres:///db1"), "got: {env}");

        // Cannot destroy while bound.
        assert!(svc.deprovision("db1").is_err());

        svc.unbind("db1", "myapp").unwrap();
        let env = std::fs::read_to_string(paths.env_root.join("myapp/ENV")).unwrap();
        assert!(
            !env.contains("DATABASE_URL"),
            "unbind should remove the key"
        );

        svc.deprovision("db1").unwrap();
        assert_eq!(svc.list().len(), 0);
        assert!(!paths.data_root.join("addons/fakedb/db1").exists());
    }

    #[test]
    fn bind_preserves_unrelated_env_lines() {
        let (_tmp, paths) = setup();
        let svc = AddonService::new(&paths);
        let env_file = paths.env_root.join("myapp/ENV");
        std::fs::create_dir_all(env_file.parent().unwrap()).unwrap();
        std::fs::write(&env_file, "PORT=8080\nKEEP=$PORT\n").unwrap();

        svc.provision("fakedb", "db1").unwrap();
        svc.bind("db1", "myapp").unwrap();

        let env = std::fs::read_to_string(&env_file).unwrap();
        // The pre-existing $VAR line is preserved verbatim (not expanded).
        assert!(env.contains("KEEP=$PORT"), "got: {env}");
        assert!(env.contains("PORT=8080"));
        assert!(env.contains("DATABASE_URL=postgres:///db1"));
    }

    #[test]
    fn provision_twice_is_rejected() {
        let (_tmp, paths) = setup();
        let svc = AddonService::new(&paths);
        svc.provision("fakedb", "db1").unwrap();
        assert!(svc.provision("fakedb", "db1").is_err());
    }

    #[test]
    fn provision_unknown_plugin_errors() {
        let (_tmp, paths) = setup();
        assert!(AddonService::new(&paths).provision("nope", "x").is_err());
    }

    #[test]
    fn backup_returns_artifact_path() {
        let (_tmp, paths) = setup();
        let svc = AddonService::new(&paths);
        svc.provision("fakedb", "db1").unwrap();
        assert_eq!(svc.backup("db1").unwrap().as_deref(), Some("/tmp/db.tar"));
    }

    /// Exercise the real shipped `sqlite-volume` example bundle end-to-end, so
    /// the example stays a working reference, not just documentation.
    #[test]
    fn shipped_sqlite_volume_example_works() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        // CARGO_MANIFEST_DIR is this crate (crates/riku-plugins); the example
        // bundles live at the workspace root, two levels up.
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plugins/sqlite-volume");
        let dest = paths.plugin_root.join("sqlite-volume");
        crate::util::copy_dir_recursive(&src, &dest).unwrap();
        std::fs::set_permissions(
            dest.join("bin/addon"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();

        let svc = AddonService::new(&paths);
        svc.provision("sqlite-volume", "mydb").unwrap();

        let keys = svc.bind("mydb", "myapp").unwrap();
        assert!(keys.contains(&"DATABASE_URL".to_string()));
        let env = std::fs::read_to_string(paths.env_root.join("myapp/ENV")).unwrap();
        assert!(env.contains("DATABASE_URL=sqlite:///"), "got: {env}");
        assert!(env.contains("mydb.db"));

        let artifact = svc.backup("mydb").unwrap().expect("artifact");
        assert!(std::path::Path::new(&artifact).exists());

        svc.unbind("mydb", "myapp").unwrap();
        svc.deprovision("mydb").unwrap();
        assert!(svc.list().is_empty());
    }
}
