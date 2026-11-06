use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;

#[test]
fn deploy_tightens_a_hand_written_env_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let env_dir = paths.env_root.join("myapp");
    fs::create_dir_all(&env_dir).unwrap();
    fs::set_permissions(&env_dir, fs::Permissions::from_mode(0o755)).unwrap();
    let env_file = env_dir.join("ENV");
    fs::write(&env_file, "SECRET=x\n").unwrap();
    fs::set_permissions(&env_file, fs::Permissions::from_mode(0o644)).unwrap();
    let mut dlog = DeployLogger::new(&tmp.path().join("deploy.log")).unwrap();

    let env = load_deploy_env("myapp", &paths, &mut dlog).unwrap();

    assert_eq!(env.get("SECRET").map(String::as_str), Some("x"));
    let file_mode = fs::metadata(&env_file).unwrap().permissions().mode() & 0o777;
    let dir_mode = fs::metadata(&env_dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        (file_mode, dir_mode),
        (0o600, 0o700),
        "ENV is {file_mode:o}, its dir {dir_mode:o}"
    );
}
