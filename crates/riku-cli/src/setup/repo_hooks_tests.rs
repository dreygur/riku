use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::*;

fn bare_repo(git_root: &Path, name: &str, hook: Option<(&str, u32)>) -> std::path::PathBuf {
    let repo = git_root.join(name);
    fs::create_dir_all(repo.join("hooks")).unwrap();
    fs::write(repo.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    if let Some((text, mode)) = hook {
        let path = repo.join("hooks/post-receive");
        fs::write(&path, text).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
    }
    repo.join("hooks/post-receive")
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// Hooks an older riku wrote, or a hook with the wrong mode, get the
/// current text at 0755; a current hook and anything that is not a repo
/// are left alone, so a second run changes nothing.
#[test]
fn every_repo_hook_gets_the_current_text_once() {
    let tmp = tempfile::tempdir().unwrap();
    let current = post_receive_hook();
    let old = bare_repo(
        tmp.path(),
        "old.git",
        Some(("#!/bin/bash\nexit 0\n", 0o775)),
    );
    let loose = bare_repo(tmp.path(), "loose.git", Some((&current, 0o777)));
    let missing = bare_repo(tmp.path(), "missing.git", None);
    let fine = bare_repo(tmp.path(), "fine.git", Some((&current, 0o755)));
    fs::create_dir_all(tmp.path().join("notes")).unwrap();

    let first = refresh_repo_hooks(tmp.path());
    assert_eq!(first.updated, 3, "old, loose and missing need the hook");
    assert!(first.failed.is_empty(), "{:?}", first.failed);
    for hook in [&old, &loose, &missing, &fine] {
        assert_eq!(
            fs::read_to_string(hook).unwrap(),
            current,
            "{}",
            hook.display()
        );
        assert_eq!(mode(hook), 0o755, "{}", hook.display());
    }
    assert!(!tmp.path().join("notes/hooks").exists());

    let second = refresh_repo_hooks(tmp.path());
    assert_eq!(second.updated, 0, "nothing changed since the first run");
}

/// One repo riku cannot write does not stop the others. Root writes into
/// a 0555 directory anyway, so the case cannot be built as root.
#[test]
fn a_hook_riku_cannot_write_is_skipped_and_named() {
    if nix::unistd::geteuid().is_root() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let locked = bare_repo(tmp.path(), "locked.git", Some(("#!/bin/bash\n", 0o755)));
    let other = bare_repo(tmp.path(), "other.git", Some(("#!/bin/bash\n", 0o755)));
    let hooks_dir = locked.parent().unwrap();
    fs::set_permissions(hooks_dir, fs::Permissions::from_mode(0o555)).unwrap();

    let result = refresh_repo_hooks(tmp.path());
    fs::set_permissions(hooks_dir, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(result.updated, 1);
    assert_eq!(result.failed.len(), 1, "{:?}", result.failed);
    assert!(
        result.failed[0].0.ends_with("locked.git"),
        "{:?}",
        result.failed
    );
    assert_eq!(fs::read_to_string(&other).unwrap(), post_receive_hook());
}
