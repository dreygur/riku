use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::ready_for;
use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::nginx::RouteChange;
use crate::util::backend::Backend;

/// A host with an nginx program (never run here) and the include in place.
fn ready_host(dir: &Path) -> RikuPaths {
    let mut paths = RikuPaths::for_tests(dir);
    paths.reload_programs.nginx = dir.join("nginx");
    fs::write(&paths.reload_programs.nginx, "").unwrap();
    fs::create_dir_all(&paths.nginx_root).unwrap();
    crate::nginx::write_include_file(&paths).unwrap();
    paths
}

fn apply_myapp(dir: &Path, paths: &RikuPaths) -> anyhow::Result<()> {
    crate::router::apply(
        "myapp",
        dir,
        &HashMap::new(),
        paths,
        &[Backend::loopback(5000)],
    )
}

#[test]
fn deploy_refuses_while_old_riku_links_remain_and_writes_no_config() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = ready_host(tmp.path());
    fs::create_dir_all(&paths.nginx_sites_enabled).unwrap();
    std::os::unix::fs::symlink(
        paths.nginx_root.join("myapp.conf"),
        paths.nginx_sites_enabled.join("myapp.conf"),
    )
    .unwrap();

    let err = apply_myapp(tmp.path(), &paths).expect_err("deploy went ahead with an old link");

    assert!(err.to_string().contains("sudo riku init"), "{err}");
    assert!(
        !paths.nginx_root.join("myapp.conf").exists(),
        "a config was written before the refusal"
    );
}

/// The orphan is what a destroy that failed partway leaves. Removing it
/// must still pass the guard, or nothing could clean it up.
#[test]
fn an_orphan_conf_refuses_deploys_but_not_its_own_removal() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = ready_host(tmp.path());
    fs::write(paths.nginx_root.join("ghost.conf"), "server {}\n").unwrap();

    let err = apply_myapp(tmp.path(), &paths).expect_err("deploy went ahead with an orphan");

    assert!(err.to_string().contains("ghost.conf"), "{err}");
    assert!(!paths.nginx_root.join("myapp.conf").exists());
    let removal = ready_for(&Router::Nginx, &paths, RouteChange::Remove);
    assert!(removal.is_ok(), "removal refused: {removal:?}");
}
