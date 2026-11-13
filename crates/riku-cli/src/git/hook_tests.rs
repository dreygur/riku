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

/// A bare repo whose `main` went from a first commit to a second one, as a
/// push leaves it before the hook runs. Returns the repo and both shas.
fn pushed_twice(tmp: &Path) -> (std::path::PathBuf, String, String) {
    let repo = tmp.join("myapp.git");
    let work = tmp.join("work");
    fs::create_dir_all(&work).unwrap();
    git(tmp, &["init", "--quiet", "--bare", repo.to_str().unwrap()]);
    git(&work, &["init", "--quiet"]);
    git(
        &work,
        &["commit", "--quiet", "--allow-empty", "-m", "first"],
    );
    let first = head_of(&work, "HEAD");
    git(
        &work,
        &["commit", "--quiet", "--allow-empty", "-m", "second"],
    );
    let second = head_of(&work, "HEAD");
    git(
        &work,
        &[
            "push",
            "--quiet",
            repo.to_str().unwrap(),
            "HEAD:refs/heads/main",
        ],
    );
    (repo, first, second)
}

fn head_of(git_dir_or_work: &Path, name: &str) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "--verify", "-q", name])
        .current_dir(git_dir_or_work)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn failed() -> Result<()> {
    Err(anyhow!("release command exited with code 1"))
}

#[test]
fn a_failed_deploy_puts_the_branch_back_where_it_was() {
    let tmp = TempDir::new().unwrap();
    let (repo, first, second) = pushed_twice(tmp.path());
    let pushed = PushedRef {
        oldrev: &first,
        newrev: &second,
        refname: "refs/heads/main",
    };

    assert!(deploy_or_roll_back(&repo, &pushed, failed).is_err());

    assert_eq!(
        head_of(&repo, "refs/heads/main"),
        first,
        "the branch still names the failed commit"
    );
}

#[test]
fn a_failed_first_push_removes_the_branch() {
    let tmp = TempDir::new().unwrap();
    let (repo, _, second) = pushed_twice(tmp.path());
    let pushed = PushedRef {
        oldrev: ZERO,
        newrev: &second,
        refname: "refs/heads/main",
    };

    assert!(deploy_or_roll_back(&repo, &pushed, failed).is_err());

    assert_eq!(
        head_of(&repo, "refs/heads/main"),
        "",
        "the branch of a failed first push survived"
    );
}

#[test]
fn a_successful_deploy_or_a_moved_branch_keeps_the_branch() {
    let tmp = TempDir::new().unwrap();
    let (repo, first, second) = pushed_twice(tmp.path());
    let pushed = PushedRef {
        oldrev: &first,
        newrev: &second,
        refname: "refs/heads/main",
    };

    deploy_or_roll_back(&repo, &pushed, || Ok(())).unwrap();
    assert_eq!(head_of(&repo, "refs/heads/main"), second);

    let raced = PushedRef {
        oldrev: &second,
        newrev: &first,
        refname: "refs/heads/main",
    };
    assert!(deploy_or_roll_back(&repo, &raced, failed).is_err());
    assert_eq!(
        head_of(&repo, "refs/heads/main"),
        second,
        "a branch another push moved was rolled back"
    );
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
