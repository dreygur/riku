use super::*;

#[test]
fn a_failed_deploy_fails_the_push_unless_receive_pack_failed_first() {
    let status = NamedTempFile::new().unwrap();
    assert_eq!(push_outcome(Some(0), status.path()).0, 0, "clean push");
    assert_eq!(
        push_outcome(Some(128), status.path()).0,
        128,
        "receive-pack's own failure keeps its code"
    );

    fs::write(status.path(), "build failed").unwrap();
    assert_eq!(
        push_outcome(Some(0), status.path()),
        (1, None),
        "a recorded deploy failure fails the push, and the hook already showed it, so it is not printed again"
    );
}

#[test]
fn an_unreadable_deploy_result_fails_the_push_and_says_why() {
    let dir = tempfile::tempdir().unwrap();

    let (code, message) = push_outcome(Some(0), &dir.path().join("gone"));

    assert_eq!(code, 1, "an unknown deploy result must not pass as success");
    assert!(
        message.is_some_and(|m| m.contains("cannot read the deploy result")),
        "nobody else reported this, so the push must"
    );
}

#[test]
fn a_push_to_app_dot_git_still_reaches_its_repository() {
    assert_eq!(repo_dir_name("shop").unwrap(), "shop");
    assert_eq!(repo_dir_name("shop.git").unwrap(), "shop.git");
    assert_eq!(repo_dir_name("shop.git/").unwrap(), "shop.git");
    assert!(
        repo_dir_name("shop.git.git").is_err(),
        "only one .git comes off, and app shop.git is refused"
    );
}
