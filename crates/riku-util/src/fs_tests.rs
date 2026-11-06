use std::os::unix::fs::PermissionsExt;

use super::*;

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn a_private_write_replaces_a_readable_file_with_an_owner_only_one() {
    let tmp = tempfile::TempDir::new().unwrap();
    let env_file = tmp.path().join("ENV");
    fs::write(&env_file, "OLD=1\n").unwrap();
    fs::set_permissions(&env_file, fs::Permissions::from_mode(0o644)).unwrap();
    let stale_tmp = tmp.path().join(format!(".ENV.tmp-{}", std::process::id()));
    fs::write(&stale_tmp, "").unwrap();
    fs::set_permissions(&stale_tmp, fs::Permissions::from_mode(0o644)).unwrap();

    write_private(&env_file, b"SECRET=1\n").unwrap();

    assert_eq!(fs::read_to_string(&env_file).unwrap(), "SECRET=1\n");
    assert_eq!(
        mode(&env_file),
        0o600,
        "ENV file is mode {:o}",
        mode(&env_file)
    );
}

#[test]
fn a_private_dir_is_tightened_when_it_already_exists() {
    let tmp = tempfile::TempDir::new().unwrap();
    let env_dir = tmp.path().join("envs").join("shop");
    fs::create_dir_all(&env_dir).unwrap();
    fs::set_permissions(&env_dir, fs::Permissions::from_mode(0o755)).unwrap();

    create_private_dir(&env_dir).unwrap();

    assert_eq!(
        mode(&env_dir),
        0o700,
        "env dir is mode {:o}",
        mode(&env_dir)
    );
}
