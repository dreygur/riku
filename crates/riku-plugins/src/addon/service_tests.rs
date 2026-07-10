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
