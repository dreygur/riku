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

    cmd_destroy(&paths, "shop", DestroyMode::Safe).unwrap();

    assert!(
        fs::symlink_metadata(&link).is_err(),
        "repo link {} survived destroy",
        link.display()
    );
    assert!(user_repo.exists(), "destroy followed the link");
}

fn container_app_without_engine(root: &Path) -> RikuPaths {
    let paths = RikuPaths::for_tests(root);
    let app_dir = paths.app_root.join("shop");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(
        app_dir.join("compose.yaml"),
        "services:\n  web:\n    image: nginx:alpine\n",
    )
    .unwrap();
    fs::create_dir_all(paths.env_root.join("shop")).unwrap();
    fs::write(paths.env_root.join("shop").join("ENV"), "").unwrap();
    fs::create_dir_all(&paths.riku_root).unwrap();
    fs::write(
        &paths.container_engine_file,
        root.join("no-such-engine").display().to_string(),
    )
    .unwrap();
    paths
}

#[test]
fn destroy_refuses_to_orphan_containers_when_the_engine_is_unreachable() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = container_app_without_engine(tmp.path());

    let err = cmd_destroy(&paths, "shop", DestroyMode::Safe).unwrap_err();

    let message = format!("{err:#}");
    assert!(
        message.contains("unreachable") && message.contains("orphaned"),
        "{message}"
    );
    assert!(
        paths.app_root.join("shop").exists(),
        "a refused destroy removed the app"
    );
}

#[test]
fn forced_destroy_removes_the_app_when_the_engine_is_unreachable() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = container_app_without_engine(tmp.path());

    cmd_destroy(&paths, "shop", DestroyMode::Force).unwrap();

    assert!(
        !paths.app_root.join("shop").exists(),
        "forced destroy left the app directory"
    );
}
