//! Fixtures shared by this crate's test modules.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::config::RikuPaths;

pub(crate) const ACCEPT: &str = "exit 0\n";
pub(crate) const REJECT: &str = "echo 'emerg: unexpected end of file' >&2\nexit 1\n";

/// Test paths with `nginx_root` created and `nginx -t` answered by a
/// script with `body`, so no test depends on the host's nginx.
pub(crate) fn paths_with_nginx(dir: &Path, body: &str) -> RikuPaths {
    let mut paths = RikuPaths::for_tests(dir);
    fs::create_dir_all(&paths.nginx_root).unwrap();
    let nginx = dir.join("mock_nginx");
    fs::write(&nginx, format!("#!/bin/sh\n{body}")).unwrap();
    fs::set_permissions(&nginx, fs::Permissions::from_mode(0o755)).unwrap();
    paths.reload_programs.nginx = nginx;
    paths
}

/// Generate `myapp`'s config for `env` under `dir`, with `nginx -t`
/// accepting, and return the live config text.
pub(crate) fn generate_live(dir: &Path, paths: &RikuPaths, env: &[(&str, &str)]) -> String {
    let app_path = dir.join("myapp");
    fs::create_dir_all(&app_path).unwrap();
    let env: HashMap<String, String> = env
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    crate::generate_nginx_config(
        "myapp",
        &app_path,
        &env,
        &crate::default_backends("myapp", &env, paths).unwrap(),
        paths,
    )
    .unwrap();
    fs::read_to_string(paths.nginx_root.join("myapp.conf")).unwrap()
}
