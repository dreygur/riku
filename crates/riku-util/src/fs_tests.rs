use std::os::unix::fs::PermissionsExt;

use super::*;

#[test]
fn a_private_dir_is_tightened_when_it_already_exists() {
    let tmp = tempfile::TempDir::new().unwrap();
    let env_dir = tmp.path().join("envs").join("shop");
    fs::create_dir_all(&env_dir).unwrap();
    fs::set_permissions(&env_dir, fs::Permissions::from_mode(0o755)).unwrap();

    create_private_dir(&env_dir).unwrap();

    let mode = fs::metadata(&env_dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700, "env dir is mode {mode:o}");
}
