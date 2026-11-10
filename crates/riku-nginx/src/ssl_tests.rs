use super::*;
use tempfile::TempDir;

fn make_paths(tmp: &TempDir) -> crate::config::RikuPaths {
    let paths = crate::config::RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.nginx_root).unwrap();
    std::fs::create_dir_all(&paths.acme_root).unwrap();
    paths
}

/// An acme.sh stand-in that appends its argv to `argv.log` and runs `body`.
fn mock_acme_sh(tmp: &TempDir, paths: &crate::config::RikuPaths, body: &str) -> std::path::PathBuf {
    std::fs::create_dir_all(&paths.acme_sh_root).unwrap();
    let argv_log = tmp.path().join("argv.log");
    let acme_sh = paths.acme_sh_root.join("acme.sh");
    std::fs::write(
        &acme_sh,
        format!("#!/bin/sh\necho \"$@\" >> {}\n{body}\n", argv_log.display()),
    )
    .unwrap();
    std::fs::set_permissions(&acme_sh, fs::Permissions::from_mode(0o755)).unwrap();
    argv_log
}

fn domains(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

#[test]
fn an_existing_key_is_kept_and_made_owner_only() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let key = paths.nginx_root.join("myapp.key");
    std::fs::write(&key, "key-data").unwrap();
    std::fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::write(paths.nginx_root.join("myapp.crt"), "crt-data").unwrap();

    assert!(ensure_ssl_certificates("myapp", &domains(&["example.com"]), &paths).unwrap());

    assert_eq!(std::fs::read_to_string(&key).unwrap(), "key-data");
    let mode = fs::metadata(&key).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "private key is mode {mode:o}");
}

#[test]
fn a_self_signed_pair_retries_acme_with_the_renew_hook_and_survives_a_failure() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let argv_log = mock_acme_sh(&tmp, &paths, "exit 1");
    let key = paths.nginx_root.join("myapp.key");
    std::fs::write(&key, "self-signed-key").unwrap();
    std::fs::write(paths.nginx_root.join("myapp.crt"), "self-signed-crt").unwrap();

    assert!(ensure_ssl_certificates("myapp", &domains(&["example.com"]), &paths).unwrap());

    let argv = std::fs::read_to_string(&argv_log).expect("acme.sh was not run");
    assert!(
        argv.contains("--reloadcmd /usr/local/bin/riku router-renew myapp"),
        "acme.sh argv was: {argv}"
    );
    assert_eq!(std::fs::read_to_string(&key).unwrap(), "self-signed-key");
}

#[test]
fn an_issued_certificate_gets_the_renew_hook_once() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let argv_log = mock_acme_sh(&tmp, &paths, "");
    for suffix in ["key", "crt", "fullchain.crt"] {
        std::fs::write(paths.nginx_root.join(format!("myapp.{suffix}")), "x").unwrap();
    }
    let names = domains(&["example.com"]);

    assert!(ensure_ssl_certificates("myapp", &names, &paths).unwrap());
    assert!(ensure_ssl_certificates("myapp", &names, &paths).unwrap());

    let argv = std::fs::read_to_string(&argv_log).expect("acme.sh was not run");
    assert_eq!(argv.lines().count(), 1, "acme.sh ran again: {argv}");
    assert!(
        argv.starts_with("--install-cert -d example.com --key-file "),
        "acme.sh argv was: {argv}"
    );
    assert!(
        argv.contains("--reloadcmd /usr/local/bin/riku router-renew myapp"),
        "acme.sh argv was: {argv}"
    );
}

fn write_own_pair(paths: &crate::config::RikuPaths) {
    for suffix in ["key", "crt", "fullchain.crt", "own-cert"] {
        std::fs::write(paths.nginx_root.join(format!("myapp.{suffix}")), "own").unwrap();
    }
}

#[test]
fn an_own_certificate_is_never_issued_replaced_or_hooked() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let argv_log = mock_acme_sh(&tmp, &paths, "exit 0");
    write_own_pair(&paths);

    assert!(ensure_ssl_certificates("myapp", &domains(&["example.com"]), &paths).unwrap());

    assert!(
        !argv_log.exists(),
        "acme.sh ran for an own certificate: {:?}",
        std::fs::read_to_string(&argv_log)
    );
    let key = paths.nginx_root.join("myapp.key");
    assert_eq!(std::fs::read_to_string(&key).unwrap(), "own");
    let mode = fs::metadata(&key).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "private key is mode {mode:o}");
}

#[test]
fn an_own_marker_without_a_pair_is_an_error_not_a_self_signed_certificate() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    std::fs::write(paths.nginx_root.join("myapp.own-cert"), "").unwrap();

    let outcome = ensure_ssl_certificates("myapp", &domains(&["example.com"]), &paths);

    assert!(outcome.is_err(), "got {outcome:?}");
    assert!(!paths.nginx_root.join("myapp.crt").exists());
}

#[test]
fn one_certificate_covers_every_domain() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let key = paths.nginx_root.join("myapp.key");
    let argv_log = mock_acme_sh(&tmp, &paths, &format!(": > {}", key.display()));

    let names = domains(&["a.example", "b.example"]);
    assert!(ensure_ssl_certificates("myapp", &names, &paths).unwrap());

    let argv = std::fs::read_to_string(&argv_log).expect("acme.sh was not run");
    assert_eq!(
        argv.lines().count(),
        1,
        "acme.sh ran more than once: {argv}"
    );
    assert!(
        argv.starts_with("--issue -d a.example -d b.example -w "),
        "acme.sh argv was: {argv}"
    );
}

#[test]
fn a_failed_shared_issue_falls_back_to_one_domain() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let key = paths.nginx_root.join("myapp.key");
    let argv_log = mock_acme_sh(
        &tmp,
        &paths,
        &format!(
            "case \"$*\" in *b.example*) exit 1 ;; esac\n: > {}",
            key.display()
        ),
    );

    let names = domains(&["a.example", "b.example"]);
    assert!(ensure_ssl_certificates("myapp", &names, &paths).unwrap());

    let argv = std::fs::read_to_string(&argv_log).expect("acme.sh was not run");
    let issues: Vec<&str> = argv.lines().collect();
    assert_eq!(issues.len(), 2, "acme.sh argv was: {argv}");
    assert!(
        issues[0].starts_with("--issue -d a.example -d b.example "),
        "{argv}"
    );
    assert!(issues[1].starts_with("--issue -d a.example -w "), "{argv}");
}

#[test]
fn the_renew_hook_stops_at_the_certificate_acme_sh_has() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let argv_log = mock_acme_sh(&tmp, &paths, "case \"$*\" in *b.example*) exit 1 ;; esac");
    for suffix in ["key", "crt", "fullchain.crt"] {
        std::fs::write(paths.nginx_root.join(format!("myapp.{suffix}")), "x").unwrap();
    }
    let names = domains(&["a.example", "b.example"]);

    assert!(ensure_ssl_certificates("myapp", &names, &paths).unwrap());
    assert!(ensure_ssl_certificates("myapp", &names, &paths).unwrap());

    let argv = std::fs::read_to_string(&argv_log).expect("acme.sh was not run");
    assert_eq!(
        argv.lines().collect::<Vec<_>>().len(),
        1,
        "the hook should be set once, on a.example's certificate: {argv}"
    );
}

#[test]
fn rejects_an_app_name_that_could_reach_the_reload_shell() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let err = ensure_ssl_certificates("app; touch /tmp/x", &domains(&["example.com"]), &paths)
        .unwrap_err();

    assert!(err.to_string().contains("not a valid app name"), "{err}");
}

#[test]
fn forgetting_acme_stops_renewals_writing_over_the_app_files() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let argv_log = mock_acme_sh(&tmp, &paths, "");
    std::fs::create_dir_all(paths.acme_sh_root.join("a.example_ecc")).unwrap();
    let hook = paths.nginx_root.join("myapp.renew-hook");
    std::fs::write(&hook, "").unwrap();
    std::fs::create_dir_all(&paths.acme_cert_links).unwrap();
    let link = paths.acme_cert_links.join("myapp");
    std::os::unix::fs::symlink(paths.acme_sh_root.join("a.example_ecc"), &link).unwrap();

    crate::forget_acme("myapp", &domains(&["b.example"]), &paths);

    let argv = std::fs::read_to_string(&argv_log).expect("acme.sh was not run");
    assert_eq!(argv.trim(), "--remove -d a.example --ecc", "{argv}");
    assert!(!hook.exists(), "the renew hook marker is left");
    assert!(
        std::fs::symlink_metadata(&link).is_err(),
        "the acme link is left"
    );
}

#[test]
fn a_skipped_issue_installs_the_certificate_acme_sh_already_has() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let key = paths.nginx_root.join("myapp.key");
    let argv_log = mock_acme_sh(
        &tmp,
        &paths,
        &format!(
            "case \"$1\" in --issue) exit 2 ;; esac\n: > {}",
            key.display()
        ),
    );

    let names = domains(&["a.example"]);
    assert!(ensure_ssl_certificates("myapp", &names, &paths).unwrap());

    let argv = std::fs::read_to_string(&argv_log).expect("acme.sh was not run");
    let calls: Vec<&str> = argv.lines().collect();
    assert_eq!(calls.len(), 2, "{argv}");
    assert!(
        calls[1].starts_with("--install-cert -d a.example "),
        "{argv}"
    );
    assert!(
        !paths.nginx_root.join("myapp.crt").exists() || key.exists(),
        "a self-signed pair was made"
    );
}

#[test]
#[ignore = "requires openssl binary on PATH"]
fn test_generate_self_signed_cert_produces_files() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let result = ensure_ssl_certificates("testapp", &domains(&["localhost"]), &paths);
    assert!(result.is_ok());
    assert!(paths.nginx_root.join("testapp.key").exists());
    assert!(paths.nginx_root.join("testapp.crt").exists());
}
