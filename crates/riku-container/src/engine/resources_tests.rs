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
fn volume_labels_reads_the_map_and_treats_null_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    // Shapes captured from `docker volume inspect --format '{{json .Labels}}'`
    // (docker 29.8.2): keys sorted, `null` for a volume without labels.
    let (engine, log) = logging_engine(dir.path(), r#"echo '{"a":"b","riku.app":"shop"}'"#);
    let labels = engine.volume_labels("shop_data").unwrap();
    assert_eq!(labels["riku.app"], "shop");
    assert_eq!(labels.len(), 2);
    assert_eq!(
        logged(&log),
        ["volume inspect --format {{json .Labels}} shop_data"]
    );

    let bare = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(bare.path(), "echo null");
    assert!(engine.volume_labels("shop_data").unwrap().is_empty());

    assert!(engine.volume_labels("-shop").is_err());
}

#[test]
fn remove_network_skips_an_absent_network_and_removes_a_present_one() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(dir.path(), "");
    engine.remove_network("riku-myapp").unwrap();
    assert_eq!(logged(&log).len(), 1, "removed an absent network");

    let present = tempfile::tempdir().unwrap();
    let (engine, log) = logging_engine(present.path(), &listing("network ls", "riku-myapp"));
    engine.remove_network("riku-myapp").unwrap();
    assert_eq!(logged(&log)[1], "network rm riku-myapp");

    let failing_dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(failing_dir.path(), &failing("network ls"));
    let err = engine.remove_network("riku-myapp").unwrap_err().to_string();
    assert!(err.contains("cannot reach engine"), "{err}");
}

#[test]
fn volume_exists_reports_engine_failure_as_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _log) = logging_engine(dir.path(), &failing("volume ls"));

    let err = engine.volume_exists("shop_data").unwrap_err().to_string();

    assert!(err.contains("cannot reach engine"), "{err}");
}
