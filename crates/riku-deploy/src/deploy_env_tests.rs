use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;

fn app(tmp: &tempfile::TempDir) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("myapp")).unwrap();
    fs::create_dir_all(paths.env_root.join("myapp")).unwrap();
    paths
}

#[test]
fn riku_toml_domains_reach_the_router_env_despite_a_broken_procfile() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = app(&tmp);
    fs::write(
        paths.app_settings_repo("myapp"),
        "[routing]\ndomains = [\"shop.example\"]\n",
    )
    .unwrap();
    fs::write(
        paths.app_root.join("myapp").join("Procfile"),
        "not a procfile\n",
    )
    .unwrap();

    let env = crate::supervisor_ctl::read_app_env("myapp", &paths).unwrap();

    assert_eq!(
        env.get("RIKU_DOMAINS").map(String::as_str),
        Some("shop.example")
    );
}

#[test]
fn container_app_refuses_riku_toml_processes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = app(&tmp);
    fs::write(
        paths.app_settings_repo("myapp"),
        "[processes]\nweb = \"node server.js\"\n",
    )
    .unwrap();

    let message = refuse_container_processes("myapp", &paths)
        .unwrap_err()
        .to_string();

    assert!(message.contains("container apps"), "{message}");
}

#[test]
fn riku_toml_processes_are_the_deploy_process_list() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = app(&tmp);
    fs::write(
        paths.app_settings_repo("myapp"),
        "[processes]\nworker = \"node jobs.js\"\n",
    )
    .unwrap();
    let mut dlog = DeployLogger::new(&tmp.path().join("deploy.log")).unwrap();

    let processes = load_deploy_processes("myapp", &paths, &mut dlog)
        .unwrap()
        .unwrap();

    assert_eq!(
        processes.get("worker").map(String::as_str),
        Some("node jobs.js")
    );
}

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
