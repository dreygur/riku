use std::os::unix::fs::{symlink, PermissionsExt};

use tempfile::TempDir;

use super::*;

#[test]
fn state_lives_under_the_riku_root_with_private_modes() {
    let root = TempDir::new().unwrap();
    let dir = agent_dir(root.path()).unwrap();
    assert!(dir.starts_with(root.path()), "{}", dir.display());
    let file = dir.join("audit.log");
    open_private(&file, Access::Append).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&file), 0o600);
}

#[test]
fn symlink_at_a_state_path_is_refused() {
    let root = TempDir::new().unwrap();
    let dir = agent_dir(root.path()).unwrap();
    let victim = root.path().join("victim");
    std::fs::write(&victim, "keep").unwrap();
    let link = dir.join("audit.log");
    symlink(&victim, &link).unwrap();
    for access in [Access::Read, Access::Rewrite, Access::Append] {
        assert!(open_private(&link, access).is_err());
    }
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
}

#[test]
fn loose_state_directory_is_tightened_to_0700() {
    let root = TempDir::new().unwrap();
    let dir = agent_dir(root.path()).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    agent_dir(root.path()).unwrap();
    let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
}

#[test]
fn state_directory_owned_by_someone_else_is_refused() {
    let root = TempDir::new().unwrap();
    let dir = agent_dir(root.path()).unwrap();
    let other_uid = std::os::unix::fs::MetadataExt::uid(&std::fs::metadata(&dir).unwrap()) + 1;
    assert!(secure_dir(&dir, other_uid).is_err());
}
