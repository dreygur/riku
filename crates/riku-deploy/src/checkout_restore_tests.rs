use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::RikuPaths;
use crate::releases::ReleaseLog;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
        .args(args)
        .env_remove("GIT_DIR")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit(dir: &Path, file: &str, message: &str) -> String {
    fs::write(dir.join(file), message).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

/// An app checkout with a released commit and a later commit no runtime
/// can build.
fn app_with_release(paths: &RikuPaths) -> (String, String) {
    let checkout = paths.app_root.join("shop");
    fs::create_dir_all(&checkout).unwrap();
    git(&checkout, &["init", "-q"]);
    let good = commit(&checkout, "app.txt", "good");
    ReleaseLog::new(paths).record("shop", &good).unwrap();
    let broken = commit(&checkout, "app.txt", "broken");
    (good, broken)
}

#[test]
fn failed_deploy_puts_the_checkout_back_at_the_last_release() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let (good, broken) = app_with_release(&paths);

    let deployed = crate::do_deploy("shop", &paths, &Default::default(), Some(&broken));

    assert!(
        deployed.is_err(),
        "an app no runtime detects must not deploy"
    );
    let head = git(&paths.app_root.join("shop"), &["rev-parse", "HEAD"]);
    assert_eq!(
        head, good,
        "after the failed deploy of {broken} the checkout must be back at the released {good}"
    );
}

#[test]
fn failed_first_deploy_leaves_the_checkout_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let checkout = paths.app_root.join("shop");
    fs::create_dir_all(&checkout).unwrap();
    git(&checkout, &["init", "-q"]);
    let first = commit(&checkout, "app.txt", "first");

    let deployed = crate::do_deploy("shop", &paths, &Default::default(), Some(&first));

    assert!(
        deployed.is_err(),
        "an app no runtime detects must not deploy"
    );
    let head = git(&checkout, &["rev-parse", "HEAD"]);
    assert_eq!(
        head, first,
        "with no release to go back to the pushed commit stays checked out"
    );
}
