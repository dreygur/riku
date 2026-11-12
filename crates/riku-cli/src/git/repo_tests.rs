use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

/// The hook hands the app name and repository to the riku binary with a
/// bare PATH and without reading the pushed refs itself. The test swaps the
/// installed path in the hook text for a mock.
#[test]
fn hook_passes_app_and_repo_to_the_binary() {
    let home = tempfile::tempdir().unwrap();
    let mock_riku = home.path().join("mock-riku");
    fs::write(
        &mock_riku,
        "#!/bin/sh\ncat >/dev/null\necho \"$@\" > \"$HOME/called\"\n",
    )
    .unwrap();
    fs::set_permissions(&mock_riku, fs::Permissions::from_mode(0o755)).unwrap();
    let repo = home.path().join("myapp");
    fs::create_dir_all(&repo).unwrap();
    let hook = home.path().join("post-receive");
    let original = post_receive_hook();
    let script = original.replace(crate::util::ROOT_BINARY, mock_riku.to_str().unwrap());
    assert_ne!(script, original, "hook does not name the root binary");
    fs::write(&hook, script).unwrap();

    let status = Command::new("bash")
        .arg(&hook)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", home.path())
        .current_dir(&repo)
        // The hook pipes stdin to riku; an inherited open stdin never ends.
        .stdin(std::process::Stdio::null())
        .status()
        .unwrap();

    let called = fs::read_to_string(home.path().join("called")).unwrap_or_default();
    assert!(status.success(), "hook exited with {status}");
    assert!(
        called.starts_with("git-hook myapp "),
        "mock riku got {called:?}"
    );
}

/// Without the binary the push fails with one rail error line, not the old
/// ` !     ` prefix.
#[test]
fn hook_without_the_binary_fails_with_a_rail_error_line() {
    let home = tempfile::tempdir().unwrap();
    let missing = home.path().join("no-riku");
    let hook = home.path().join("post-receive");
    let script = post_receive_hook().replace(crate::util::ROOT_BINARY, missing.to_str().unwrap());
    fs::write(&hook, script).unwrap();

    let output = Command::new("bash")
        .arg(&hook)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir(home.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!("■  riku binary not found at {}\n", missing.display())
    );
}
