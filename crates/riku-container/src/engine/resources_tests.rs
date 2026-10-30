use crate::engine::fixture::{logged, logging_engine};
use crate::types::NetworkName;

fn listing(command: &str, output: &str) -> String {
    format!("case \"$1 $2\" in \"{command}\") printf '%s\\n' '{output}';; esac")
}

fn failing(command: &str) -> String {
    format!("case \"$1 $2\" in \"{command}\") echo 'cannot reach engine' >&2; exit 1;; esac")
}

#[test]
fn ensure_network_creates_only_when_the_exact_name_is_absent() {
    let dir = tempfile::tempdir().unwrap();
    let network = NetworkName::for_app("myapp");

    let (engine, log) = logging_engine(dir.path(), &listing("network ls", "riku-myapp"));
    engine.ensure_network(&network).unwrap();
    assert_eq!(
        logged(&log).len(),
        1,
        "created an existing network: {:?}",
        logged(&log)
    );

    let other = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(other.path(), &listing("network ls", "riku-myapp-2"));
    engine.ensure_network(&network).unwrap();
    let lines = logged(&log);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[1], "network create riku-myapp");
}

#[test]
fn ensure_network_reports_engine_failure_instead_of_creating() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), &failing("network ls"));

    let err = engine
        .ensure_network(&NetworkName::for_app("myapp"))
        .unwrap_err()
        .to_string();

    assert!(err.contains("cannot reach engine"), "{err}");
    assert_eq!(logged(&log).len(), 1, "{:?}", logged(&log));
}

#[test]
fn volume_exists_matches_the_exact_name_only() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(
        dir.path(),
        &listing("volume ls", "shop_data\nshop_data_old"),
    );

    assert!(engine.volume_exists("shop_data").unwrap());
    assert!(!engine.volume_exists("shop").unwrap());
}

#[test]
fn volume_exists_reports_engine_failure_as_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), &failing("volume ls"));

    let err = engine.volume_exists("shop_data").unwrap_err().to_string();

    assert!(err.contains("cannot reach engine"), "{err}");
}
