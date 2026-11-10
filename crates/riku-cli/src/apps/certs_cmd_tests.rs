use std::cell::RefCell;
use std::fs;

use super::*;
use crate::own_cert::test_pems::{OLD_CERT, OLD_KEY, VALID_CERT, VALID_KEY};

fn paths_with_app(tmp: &tempfile::TempDir) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp.path());
    fs::create_dir_all(paths.app_root.join("myapp")).unwrap();
    fs::create_dir_all(&paths.nginx_root).unwrap();
    paths
}

fn set_domains(paths: &RikuPaths, domains: &str) {
    let dir = paths.env_root.join("myapp");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("ENV"), format!("RIKU_DOMAINS={domains}\n")).unwrap();
}

fn bundle(cert: &str, key: &str) -> String {
    format!("{cert}{key}")
}

#[test]
fn set_saves_the_pair_and_reapplies_the_route_once() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);
    let applied = RefCell::new(Vec::new());

    let outcome = install_bundle(&paths, "myapp", &bundle(VALID_CERT, VALID_KEY), |app| {
        applied.borrow_mut().push(app.to_string());
        Ok(())
    })
    .unwrap();

    assert_eq!(*applied.borrow(), ["myapp"]);
    assert!(outcome.uncovered.is_empty());
    assert_eq!(
        crate::own_cert::store::source(&paths, "myapp"),
        crate::own_cert::store::Source::Own
    );
}

#[test]
fn set_refuses_an_expired_certificate_and_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);

    let err = install_bundle(&paths, "myapp", &bundle(OLD_CERT, OLD_KEY), |_| Ok(())).unwrap_err();

    assert!(err.to_string().contains("expired"), "got: {err:#}");
    assert_eq!(fs::read_dir(&paths.nginx_root).unwrap().count(), 0);
}

#[test]
fn set_refuses_a_key_that_does_not_match_and_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);

    let outcome = install_bundle(&paths, "myapp", &bundle(VALID_CERT, OLD_KEY), |_| Ok(()));

    assert!(outcome.is_err());
    assert_eq!(fs::read_dir(&paths.nginx_root).unwrap().count(), 0);
}

#[test]
fn set_reports_domains_the_certificate_does_not_cover_but_still_installs() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);
    set_domains(&paths, "edge.example, www.shop.example, other.example");

    let outcome =
        install_bundle(&paths, "myapp", &bundle(VALID_CERT, VALID_KEY), |_| Ok(())).unwrap();

    assert_eq!(outcome.uncovered, ["other.example"]);
    assert!(paths.nginx_root.join("myapp.own-cert").exists());
}

#[test]
fn a_failed_reapply_keeps_the_files_and_says_so() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);

    let err = install_bundle(&paths, "myapp", &bundle(VALID_CERT, VALID_KEY), |_| {
        Err(anyhow!("helper down"))
    })
    .unwrap_err();

    let text = format!("{err:#}");
    assert!(
        text.contains("saved") && text.contains("helper down"),
        "got: {text}"
    );
    assert!(paths.nginx_root.join("myapp.own-cert").exists());
}

#[test]
fn certs_commands_refuse_a_bad_or_unknown_app() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);

    assert!(cmd_certs_show(&paths, "../x").is_err());
    assert!(cmd_certs_unset(&paths, "ghost").is_err());
}

#[test]
fn show_names_the_source_names_and_expiry_without_the_key() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);
    assert_eq!(show_lines(&paths, "myapp").unwrap()[0], "source: none");

    install_bundle(&paths, "myapp", &bundle(VALID_CERT, VALID_KEY), |_| Ok(())).unwrap();
    let lines = show_lines(&paths, "myapp").unwrap();

    assert_eq!(lines[0], "source: own certificate");
    assert_eq!(lines[1], "names: edge.example, *.shop.example");
    assert!(
        lines[2].starts_with("expires: 2126-01-01"),
        "got: {}",
        lines[2]
    );
    assert!(!lines.join("\n").contains("PRIVATE"));
}

#[test]
fn unset_removes_an_own_pair_and_refuses_one_riku_issued() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);
    fs::write(paths.nginx_root.join("myapp.crt"), "x").unwrap();
    fs::write(paths.nginx_root.join("myapp.fullchain.crt"), "x").unwrap();
    assert!(remove_own(&paths, "myapp", |_| Ok(())).is_err());
    assert!(
        paths.nginx_root.join("myapp.crt").exists(),
        "issued pair deleted"
    );

    install_bundle(&paths, "myapp", &bundle(VALID_CERT, VALID_KEY), |_| Ok(())).unwrap();
    remove_own(&paths, "myapp", |_| Ok(())).unwrap();

    assert_eq!(fs::read_dir(&paths.nginx_root).unwrap().count(), 0);
}

#[test]
fn unset_puts_the_route_back_on_riku_certificates_at_once() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_with_app(&tmp);
    install_bundle(&paths, "myapp", &bundle(VALID_CERT, VALID_KEY), |_| Ok(())).unwrap();
    let applied = RefCell::new(Vec::new());

    remove_own(&paths, "myapp", |app| {
        assert!(
            !paths.nginx_root.join("myapp.own-cert").exists(),
            "the route was applied before the marker went"
        );
        applied.borrow_mut().push(app.to_string());
        Ok(())
    })
    .unwrap();

    assert_eq!(applied.into_inner(), ["myapp"]);
}
