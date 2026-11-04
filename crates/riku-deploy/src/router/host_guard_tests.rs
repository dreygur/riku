use std::collections::HashMap;
use std::fs;

use crate::config::RikuPaths;
use crate::util::backend::Backend;

#[test]
fn an_orphan_conf_refuses_deploys_and_writes_no_config() {
    let tmp = tempfile::tempdir().unwrap();
    let mut paths = RikuPaths::for_tests(tmp.path());
    paths.reload_programs.nginx = tmp.path().join("nginx");
    fs::write(&paths.reload_programs.nginx, "").unwrap();
    fs::create_dir_all(&paths.nginx_root).unwrap();
    fs::write(paths.nginx_root.join("ghost.conf"), "server {}\n").unwrap();

    let err = crate::router::apply(
        "myapp",
        tmp.path(),
        &HashMap::new(),
        &paths,
        &[Backend::loopback(5000)],
    )
    .expect_err("deploy went ahead with an orphan");

    assert!(err.to_string().contains("ghost.conf"), "{err}");
    assert!(!paths.nginx_root.join("myapp.conf").exists());
}
