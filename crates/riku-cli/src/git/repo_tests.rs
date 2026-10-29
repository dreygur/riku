use super::*;

/// A push over SSH runs the hook with a bare PATH that has no
/// `~/.local/bin`, where root init installs riku.
#[test]
fn hook_finds_riku_in_local_bin_when_not_on_path() {
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join(".local/bin");
    fs::create_dir_all(&bin).unwrap();
    let mock_riku = bin.join("riku");
    fs::write(
        &mock_riku,
        "#!/bin/sh\ncat >/dev/null\necho \"$@\" > \"$HOME/called\"\n",
    )
    .unwrap();
    fs::set_permissions(&mock_riku, fs::Permissions::from_mode(0o755)).unwrap();
    let repo = home.path().join("myapp");
    fs::create_dir_all(&repo).unwrap();
    let hook = home.path().join("post-receive");
    fs::write(&hook, POST_RECEIVE_HOOK).unwrap();

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
