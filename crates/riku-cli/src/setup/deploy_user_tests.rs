use std::os::unix::fs::PermissionsExt;

use super::*;

/// The current user stands in for the deploy user: switching to your own
/// uid needs no root. Whether root really drops to the deploy user cannot
/// be tested without root and a second account.
fn current_user(home: &Path) -> DeployUser {
    let entry = User::from_uid(Uid::current()).unwrap().unwrap();
    DeployUser {
        name: entry.name,
        home: home.to_path_buf(),
        group: group_name(entry.gid).unwrap(),
        uid: entry.uid,
        gid: entry.gid,
    }
}

#[test]
fn file_and_dir_changes_run_through_the_user() {
    let tmp = tempfile::tempdir().unwrap();
    let user = current_user(tmp.path());
    let dir = tmp.path().join(".riku/caddy");
    let file = tmp.path().join(".riku/ROUTER");

    user.create_dir(&dir).unwrap();
    user.chmod("750", &dir).unwrap();
    let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o750, "site dir mode is {mode:o}");

    user.write_file(&file, "caddy\n").unwrap();
    user.write_file(&file, "nginx\n").unwrap();
    assert_eq!(
        user.read_file(&file).unwrap(),
        "nginx\n",
        "second write must replace the first"
    );

    let err = user.read_file(&tmp.path().join("missing")).unwrap_err();
    assert!(
        err.to_string().contains("cat"),
        "error should name the command: {err}"
    );
}
