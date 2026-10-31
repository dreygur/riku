use super::*;
use crate::doctor::Status;

#[test]
fn missing_import_warns() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let caddyfile = tmp.path().join("Caddyfile");
    fs::write(&caddyfile, ":80 {\n}\n").unwrap();

    let check = caddy_import(&paths, &caddyfile);
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(
        check.detail.contains(&import_line(&paths)),
        "detail should name the line to add: {}",
        check.detail
    );

    fs::write(&caddyfile, format!(":80 {{\n}}\n{}\n", import_line(&paths))).unwrap();
    assert_eq!(caddy_import(&paths, &caddyfile).status, Status::Ok);
}

#[test]
fn unreadable_router_file_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(&paths.router_file).unwrap();

    let checks = router(&paths);
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, Status::Fail, "{}", checks[0].detail);
}

#[test]
fn failed_reload_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    assert!(last_reload(&paths).is_none());

    fs::create_dir_all(&paths.riku_root).unwrap();
    let error_file = paths.riku_root.join(RELOAD_ERROR_FILE);
    fs::write(
        &error_file,
        "caddy reload failed: admin endpoint disabled\n",
    )
    .unwrap();

    let check = last_reload(&paths).expect("a recorded failure should be reported");
    assert_eq!(check.status, Status::Fail);
    assert_eq!(check.detail, "caddy reload failed: admin endpoint disabled");
}

#[test]
fn old_caddy_path_unit_warns() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(old_caddy_units(tmp.path()).is_none());

    fs::write(tmp.path().join("riku-caddy.path"), "").unwrap();
    let check = old_caddy_units(tmp.path()).expect("the old unit should be reported");
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
}
