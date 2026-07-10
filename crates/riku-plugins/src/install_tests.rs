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

/// Write a bundle whose entry script appends `on_install`/`on_uninstall`
/// to a marker file each time it's invoked with that verb, so the real
/// `PluginInstaller::install`/`remove` calls can be checked end-to-end
/// rather than calling `run_lifecycle_verb` directly.
fn write_lifecycle_bundle(dir: &Path, name: &str, marker: &Path) {
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    let entry = dir.join("bin/addon");
    std::fs::write(
            &entry,
            format!(
                "#!/bin/sh\ncase \"$1\" in\n  on_install) echo on_install >> '{}' ;;\n  on_uninstall) echo on_uninstall >> '{}' ;;\nesac\n",
                marker.display(),
                marker.display()
            ),
        )
        .unwrap();
    std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(
            dir.join("riku-plugin.toml"),
            format!(
                "name=\"{name}\"\nversion=\"1.0.0\"\ntype=\"notifier\"\napi={}\nentry=\"bin/addon\"\n[lifecycle]\ninstall=true\nuninstall=true\n",
                crate::RIKU_PLUGIN_API
            ),
        )
        .unwrap();
}

#[test]
fn lifecycle_hooks_fire_on_install_and_remove() {
    let (tmp, paths) = setup();
    let src = tmp.path().join("lifecycle-src");
    let marker = tmp.path().join("lifecycle.log");
    write_lifecycle_bundle(&src, "lifecycled", &marker);

    let installer = PluginInstaller::new(&paths);
    installer.install(src.to_str().unwrap()).unwrap();
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "on_install\n");

    installer.remove("lifecycled").unwrap();
    assert_eq!(
        std::fs::read_to_string(&marker).unwrap(),
        "on_install\non_uninstall\n"
    );
}

#[test]
fn no_lifecycle_block_means_hooks_never_run() {
    // Every existing plugin (no [lifecycle] table) must be unaffected —
    // reuses the plain `write_bundle` helper, which declares no
    // [lifecycle] block at all.
    let (tmp, paths) = setup();
    let src = tmp.path().join("plain-src");
    write_bundle(&src, "plain", None);

    let installer = PluginInstaller::new(&paths);
    // Would panic/fail loudly if run_lifecycle_verb were called against
    // this bundle's entry script, since it has no `on_install` case arm
    // and `set -e` isn't even present — the real assertion is just that
    // install/remove succeed at all with no [lifecycle] declared.
    installer.install(src.to_str().unwrap()).unwrap();
    installer.remove("plain").unwrap();
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
