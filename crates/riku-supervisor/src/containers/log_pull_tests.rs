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
    assert_eq!(written, "one\ntwo\nthree\nfour\n");
    assert_eq!(
        logged(&argv_log),
        [
            format!("logs --timestamps {ID}"),
            format!("logs --timestamps --since 2026-11-04T10:00:00.000000003Z {ID}"),
        ]
    );
}
