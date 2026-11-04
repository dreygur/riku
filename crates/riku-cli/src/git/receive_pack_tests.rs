use super::*;

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
