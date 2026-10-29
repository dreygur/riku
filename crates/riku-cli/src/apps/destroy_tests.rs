use super::*;

#[test]
fn removes_the_repo_link_but_not_the_repo_it_points_to() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let user_repo = tmp.path().join("shop.git");
    fs::create_dir_all(&user_repo).unwrap();
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    fs::create_dir_all(paths.git_root.join("shop")).unwrap();
    let link = paths.git_root.join("shop.git");
    std::os::unix::fs::symlink(&user_repo, &link).unwrap();

    cmd_destroy(&paths, "shop").unwrap();

    assert!(
        fs::symlink_metadata(&link).is_err(),
        "repo link {} survived destroy",
        link.display()
    );
    assert!(user_repo.exists(), "destroy followed the link");
}
