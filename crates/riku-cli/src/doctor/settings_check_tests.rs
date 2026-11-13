use super::*;
use crate::doctor::Status;

#[test]
fn absent_file_is_ok_with_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());

    let check = settings_file(&paths);

    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert_eq!(check.detail, "defaults");
}

fn routing_line(settings_text: &str) -> Check {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    std::fs::write(&paths.settings_file, settings_text).unwrap();
    routing(&paths).expect("a valid file gives a routing line")
}

#[test]
fn missing_routing_section_reports_the_defaults_as_ok() {
    let check = routing_line("[router]\nname = \"nginx\"\n");

    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert_eq!(check.detail, "defaults, port mode, no fallback domain");
}

#[test]
fn routing_line_names_the_fallback_domain() {
    let check = routing_line(
        "[routing]\nfallback_domain = \"{app}.{ip}.example\"\nserver_ip = \"203.0.113.10\"\n",
    );

    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert_eq!(
        check.detail,
        "fallback domain {app}.{ip}.example with 203.0.113.10"
    );
}

#[test]
fn routing_line_says_the_fallback_is_off_without_server_ip() {
    let check = routing_line("[routing]\nfallback_domain = \"{app}.{ip}.example\"\n");

    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert!(
        check.detail.contains("off (no server_ip)"),
        "{}",
        check.detail
    );
}

#[test]
fn invalid_file_fails_naming_the_key() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.riku_root).unwrap();
    std::fs::write(&paths.settings_file, "[engine]\npull_timeout = \"soon\"\n").unwrap();

    let check = settings_file(&paths);

    assert_eq!(check.status, Status::Fail);
    assert!(
        check.detail.contains("pull_timeout"),
        "detail should name the key: {}",
        check.detail
    );
}
