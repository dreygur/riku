use std::collections::HashMap;
use std::fs;

use tempfile::TempDir;

use riku_container::engine::{ContainerId, ContainerInfo, ContainerState};
use riku_container::types::{CopyIndex, ServiceName};

use super::{LogPuller, Start};
use crate::containers::mock_engine::{logged, mock_engine};

const ID: &str = "1111111111111111111111111111111111111111111111111111111111111111";

fn running_copy() -> ContainerInfo {
    ContainerInfo {
        id: ContainerId::try_from(ID).unwrap(),
        name: "myapp.web.1".to_string(),
        app: Some("myapp".to_string()),
        service: Some(ServiceName::try_from("web").unwrap()),
        copy: Some(CopyIndex::new(1).unwrap()),
        image: "myapp:v1".to_string(),
        image_id: "sha256:test".to_string(),
        host_port: None,
        restart: None,
        state: ContainerState::Running,
    }
}

/// The log file text for `lines`, each behind the engine's time for it to
/// the second, as the mocks below give it.
fn stamped(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|text| format!("2026-11-04T10:00:00Z {text}\n"))
        .collect()
}

/// The second call repeats the line stamped at its `--since` bound, as
/// engines do; it must not be written twice, and stderr lines land in
/// timestamp order between stdout lines.
#[test]
fn pulls_append_each_line_once_in_timestamp_order() {
    let dir = TempDir::new().unwrap();
    let behaviour = "case \"$1\" in logs) case \"$*\" in\n\
         *--since*) echo '2026-11-04T10:00:00.000000003Z three'; \
                    echo '2026-11-04T10:00:00.000000004Z four';;\n\
         *) echo '2026-11-04T10:00:00.000000001Z one'; \
            echo '2026-11-04T10:00:00.000000003Z three'; \
            echo '2026-11-04T10:00:00.000000002Z two' >&2;;\n\
         esac;; esac";
    let (engine, argv_log) = mock_engine(dir.path(), behaviour);
    let log_root = dir.path().join("logs");
    let mut puller = LogPuller::new(log_root.clone());
    let info = running_copy();
    let known = HashMap::from([(info.id.clone(), info)]);

    puller.tick(&engine, &known, Start::All);
    puller.tick(&engine, &known, Start::All);

    let written = fs::read_to_string(log_root.join("myapp").join("web.1.log")).unwrap();
    assert_eq!(written, stamped(&["one", "two", "three", "four"]));
    assert_eq!(
        logged(&argv_log),
        [
            format!("logs --timestamps {ID}"),
            format!("logs --timestamps --since 2026-11-04T10:00:00.000000003Z {ID}"),
        ]
    );
}

/// The first call fills the stdout cap partway through `three`. Only the
/// complete lines up to `two` are written, the later stderr line waits too,
/// and the next call continues from `two` without writing anything twice.
#[test]
fn capped_pull_writes_complete_lines_and_continues_next_call() {
    let dir = TempDir::new().unwrap();
    let behaviour = "case \"$1\" in logs) case \"$*\" in\n\
         *--since*) echo '2026-11-04T10:00:00.000000002Z two'; \
                    echo '2026-11-04T10:00:00.000000003Z three'; \
                    echo '2026-11-04T10:00:00.000000004Z four' >&2;;\n\
         *) echo '2026-11-04T10:00:00.000000001Z one'; \
            echo '2026-11-04T10:00:00.000000002Z two'; \
            echo '2026-11-04T10:00:00.000000003Z three'; \
            echo '2026-11-04T10:00:00.000000004Z four' >&2;;\n\
         esac;; esac";
    let (engine, argv_log) = mock_engine(dir.path(), behaviour);
    let log_root = dir.path().join("logs");
    let mut puller = LogPuller::new(log_root.clone());
    puller.stream_cap = 80;
    let info = running_copy();
    let known = HashMap::from([(info.id.clone(), info)]);

    puller.tick(&engine, &known, Start::All);
    let first = fs::read_to_string(log_root.join("myapp").join("web.1.log")).unwrap();
    puller.tick(&engine, &known, Start::All);

    let written = fs::read_to_string(log_root.join("myapp").join("web.1.log")).unwrap();
    assert_eq!(
        first,
        stamped(&["one", "two"]),
        "first call kept lines past the cut"
    );
    assert_eq!(written, stamped(&["one", "two", "three", "four"]));
    assert_eq!(
        logged(&argv_log)[1],
        format!("logs --timestamps --since 2026-11-04T10:00:00.000000002Z {ID}")
    );
}

/// After a rolling deploy the watch lists a replaced copy for a few seconds
/// while the engine has already removed it.
#[test]
fn container_the_engine_no_longer_has_is_not_asked_again() {
    let dir = TempDir::new().unwrap();
    let behaviour = "case \"$1\" in logs) \
         echo \"Error response from daemon: No such container: $3\" >&2; exit 1;; esac";
    let (engine, argv_log) = mock_engine(dir.path(), behaviour);
    let mut puller = LogPuller::new(dir.path().join("logs"));
    let info = running_copy();
    let known = HashMap::from([(info.id.clone(), info)]);

    puller.tick(&engine, &known, Start::All);
    puller.tick(&engine, &known, Start::All);

    assert_eq!(
        logged(&argv_log),
        [format!("logs --timestamps {ID}")],
        "a container the engine reported gone must be dropped, not retried"
    );
}

#[test]
fn refuses_to_append_through_a_symlinked_log_file() {
    let dir = TempDir::new().unwrap();
    let behaviour = "case \"$1\" in logs) echo '2026-11-04T10:00:00.000000001Z one';; esac";
    let (engine, _argv_log) = mock_engine(dir.path(), behaviour);
    let log_root = dir.path().join("logs");
    let target = dir.path().join("authorized_keys");
    fs::write(&target, "").unwrap();
    fs::create_dir_all(log_root.join("myapp")).unwrap();
    std::os::unix::fs::symlink(&target, log_root.join("myapp").join("web.1.log")).unwrap();
    let mut puller = LogPuller::new(log_root);
    let info = running_copy();
    let known = HashMap::from([(info.id.clone(), info)]);

    puller.tick(&engine, &known, Start::All);

    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "",
        "log line was written through the symlink"
    );
}
