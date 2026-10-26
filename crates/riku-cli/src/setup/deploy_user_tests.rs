use std::os::unix::fs::PermissionsExt;

use super::*;

/// The current user stands in for the deploy user: switching to your own
/// uid needs no root. Whether root really drops to the deploy user is
/// checked only by running a root init in a container.
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
    let router = tmp.path().join(".riku/ROUTER");

    user.create_dir(&dir).unwrap();
    user.chmod("750", &dir).unwrap();
    let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o750, "site dir mode is {mode:o}");

    std::fs::write(&router, "caddy\n").unwrap();
    assert_eq!(user.read_file(&router).unwrap(), "caddy\n");

    let err = user.read_file(&tmp.path().join("missing")).unwrap_err();
    assert!(
        err.to_string().contains("cat"),
        "error should name the command: {err}"
    );
}

#[test]
fn child_gets_only_allowlisted_env_and_no_terminal() {
    let tmp = tempfile::tempdir().unwrap();
    let user = current_user(tmp.path());

    let env = user.run_as("env", &[]).unwrap();
    let keys: Vec<&str> = env.lines().filter_map(|l| l.split('=').next()).collect();
    for key in &keys {
        assert!(
            PASSED_ENV.contains(key) || ["HOME", "USER", "LOGNAME"].contains(key),
            "{key} leaked into the child: {env}"
        );
    }

    // A new session has no controlling terminal, so /dev/tty cannot open.
    let tty = user.run_as(
        "sh",
        &[
            "-c",
            "if (exec 3</dev/tty) 2>/dev/null; then echo has-tty; else echo no-tty; fi",
        ],
    );
    assert_eq!(tty.unwrap().trim(), "no-tty");
}
