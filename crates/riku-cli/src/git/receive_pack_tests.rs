use super::*;

#[test]
fn a_failed_deploy_fails_the_push_unless_receive_pack_failed_first() {
    let status = NamedTempFile::new().unwrap();
    assert_eq!(push_exit_code(Some(0), status.path()), 0, "clean push");
    assert_eq!(
        push_exit_code(Some(128), status.path()),
        128,
        "receive-pack's own failure keeps its code"
    );

    fs::write(status.path(), "build failed").unwrap();
    assert_eq!(
        push_exit_code(Some(0), status.path()),
        1,
        "a recorded deploy failure must fail the push"
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
