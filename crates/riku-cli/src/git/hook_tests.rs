use super::*;
use tempfile::TempDir;

#[test]
fn clones_only_when_the_app_dir_is_not_a_checkout() {
    let tmp = TempDir::new().unwrap();
    let app_path = tmp.path().join("myapp");
    assert!(needs_clone(&app_path), "a missing app dir needs a clone");

    fs::create_dir_all(app_path.join(".git")).unwrap();
    assert!(
        !needs_clone(&app_path),
        "a checkout without a Procfile must not be cloned again"
    );
}

const SHA: &str = "4b17a4c0e1c2d3f4a5b6c7d8e9f0a1b2c3d4e5f6";
const ZERO: &str = "0000000000000000000000000000000000000000";

#[test]
fn only_the_default_branch_deploys() {
    let main = "refs/heads/main";
    assert_eq!(ref_push(main, SHA, main), RefPush::Deploy);
    assert!(matches!(
        ref_push("refs/heads/feature", SHA, main),
        RefPush::Skip(reason) if reason.contains("push to main")
    ));
    assert!(matches!(
        ref_push("refs/tags/v1", SHA, main),
        RefPush::Skip(_)
    ));
    assert!(matches!(ref_push(main, ZERO, main), RefPush::Skip(_)));
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

#[test]
fn first_pushed_branch_becomes_the_default_when_head_is_unborn() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("myapp.git");
    let work = tmp.path().join("work");
    fs::create_dir_all(&work).unwrap();
    git(
        tmp.path(),
        &["init", "--quiet", "--bare", repo.to_str().unwrap()],
    );
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/master"]);
    git(&work, &["init", "--quiet"]);
    git(
        &work,
        &["commit", "--quiet", "--allow-empty", "-m", "first"],
    );
    git(
        &work,
        &[
            "push",
            "--quiet",
            repo.to_str().unwrap(),
            "HEAD:refs/heads/main",
        ],
    );

    assert_eq!(
        default_ref(&repo, "refs/heads/main").unwrap(),
        "refs/heads/main"
    );

    git(
        &work,
        &[
            "push",
            "--quiet",
            repo.to_str().unwrap(),
            "HEAD:refs/heads/feature",
        ],
    );
    assert_eq!(
        default_ref(&repo, "refs/heads/feature").unwrap(),
        "refs/heads/main",
        "a later branch must not take over"
    );
}
