use super::*;
use tempfile::TempDir;

fn paths_for(root: &Path) -> RikuPaths {
    let paths = RikuPaths::from_dirs(root.to_path_buf(), Path::new("/home/test"));
    fs::create_dir_all(&paths.git_root).unwrap();
    paths
}

#[test]
fn rejects_traversal_sequence() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    let err = check_hook_repo(&paths, "myapp", "/srv/../etc/myapp.git").unwrap_err();
    assert!(err.to_string().contains("path traversal"), "{err}");
}

#[test]
fn accepts_only_the_apps_own_repo() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    let own = app_repo(&paths, "myapp");
    fs::create_dir_all(&own).unwrap();
    check_hook_repo(&paths, "myapp", own.to_str().unwrap()).unwrap();

    let bare_name = paths.git_root.join("myapp");
    let other = paths.git_root.join("evil.git");
    let outside = tmp.path().join("myapp.git");
    for repo in [&bare_name, &other, &outside] {
        fs::create_dir_all(repo).unwrap();
        let err = check_hook_repo(&paths, "myapp", repo.to_str().unwrap()).unwrap_err();
        assert!(
            err.to_string().contains("the app's repo is"),
            "{} must be refused: {err}",
            repo.display()
        );
    }
}

#[test]
fn rejects_a_symlinked_repo_that_leaves_the_repos_directory() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    let outside = tmp.path().join("elsewhere");
    fs::create_dir_all(&outside).unwrap();
    let own = app_repo(&paths, "myapp");
    std::os::unix::fs::symlink(&outside, &own).unwrap();

    let err = check_hook_repo(&paths, "myapp", own.to_str().unwrap()).unwrap_err();

    assert!(err.to_string().contains("the app's repo is"), "{err}");
}
