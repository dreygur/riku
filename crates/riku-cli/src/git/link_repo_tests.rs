use super::*;
use tempfile::TempDir;

fn paths_for(root: &Path) -> RikuPaths {
    let git_root = root.join("repos");
    fs::create_dir_all(&git_root).unwrap();
    RikuPaths::from_dirs(root.to_path_buf(), Path::new("/home/test"))
}

#[test]
fn rejects_traversal_sequence() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    let err = link_external_repo(&paths, "myapp", "/srv/../etc/myapp.git").unwrap_err();
    assert!(err.to_string().contains("path traversal"));
}

#[test]
fn absent_source_is_noop() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    let missing = tmp.path().join("does-not-exist");
    link_external_repo(&paths, "myapp", missing.to_str().unwrap()).unwrap();
    assert!(!paths.git_root.join("myapp.git").exists());
}

#[test]
fn rejects_non_directory_source() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    let file = tmp.path().join("myapp.git");
    fs::write(&file, b"not a dir").unwrap();
    let err = link_external_repo(&paths, "myapp", file.to_str().unwrap()).unwrap_err();
    assert!(err.to_string().contains("not a directory"));
}

#[test]
fn rejects_mismatched_basename() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    let other = tmp.path().join("evil.git");
    fs::create_dir_all(&other).unwrap();
    let err = link_external_repo(&paths, "myapp", other.to_str().unwrap()).unwrap_err();
    assert!(err.to_string().contains("basename must be"));
}

#[test]
fn links_valid_dotgit_source() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    let src = tmp.path().join("myapp.git");
    fs::create_dir_all(&src).unwrap();
    link_external_repo(&paths, "myapp", src.to_str().unwrap()).unwrap();
    let dest = paths.git_root.join("myapp.git");
    assert!(dest.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(
        fs::read_link(&dest).unwrap(),
        fs::canonicalize(&src).unwrap()
    );
}

#[test]
fn accepts_bare_app_basename() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    let src = tmp.path().join("myapp");
    fs::create_dir_all(&src).unwrap();
    link_external_repo(&paths, "myapp", src.to_str().unwrap()).unwrap();
    assert!(paths
        .git_root
        .join("myapp.git")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn refuses_to_clobber_real_destination_dir() {
    let tmp = TempDir::new().unwrap();
    let paths = paths_for(tmp.path());
    let src = tmp.path().join("myapp.git");
    fs::create_dir_all(&src).unwrap();
    // A real directory already sits at the riku destination.
    fs::create_dir_all(paths.git_root.join("myapp.git")).unwrap();
    let err = link_external_repo(&paths, "myapp", src.to_str().unwrap()).unwrap_err();
    assert!(err.to_string().contains("not a symlink"));
}
