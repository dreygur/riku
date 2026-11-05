use std::collections::HashMap;

use riku_container::engine::{ContainerId, ContainerInfo, ContainerState, ContainerStats};
use riku_container::types::{CopyIndex, ServiceName};

use riku_config::RikuPaths;

use super::container_rows;
use crate::containers::mock_actions::mock_actions;
use crate::containers::restarts::Restarts;
use crate::containers::tally::RestartTally;
use crate::health::actions::{RestartOutcome, SharedActions};
use crate::stats::types::ProcessStatus;

fn web_copy(id: &str, state: ContainerState) -> ContainerInfo {
    ContainerInfo {
        id: ContainerId::try_from(id).unwrap(),
        name: "myapp-web-1".to_string(),
        app: Some("myapp".to_string()),
        service: Some(ServiceName::try_from("web").unwrap()),
        copy: Some(CopyIndex::new(1).unwrap()),
        image: "myapp:latest".to_string(),
        image_id: "sha256:img1".to_string(),
        host_port: None,
        restart: Some("always".to_string()),
        state,
    }
}

/// The supervisor logs `restart=1` when it dispatches the restart, and the
/// tally counts it only once the report is collected on a later tick.
#[test]
fn replacement_seen_before_the_restart_report_already_counts() {
    let dead = web_copy(&"1".repeat(64), ContainerState::Exited { code: 137 });
    let fresh = web_copy(&"2".repeat(64), ContainerState::Running);
    let tmp = tempfile::tempdir().unwrap();
    let actions: SharedActions = mock_actions(RestartOutcome::Restarted);
    let mut restarts = Restarts::new();
    restarts
        .dispatch(&RikuPaths::for_tests(tmp.path()), &actions, &dead)
        .unwrap();
    let tally = RestartTally::new();

    let still_dead = container_rows(&[dead], &HashMap::new(), &tally, &restarts);
    let replaced = container_rows(&[fresh], &HashMap::new(), &tally, &restarts);

    assert_eq!(
        (still_dead[0].restart_count, replaced[0].restart_count),
        (0, 1),
        "(dead copy, its replacement)"
    );
}

#[test]
fn stats_rows_carry_engine_state() {
    let id1_str = "1111111111111111111111111111111111111111111111111111111111111111";
    let id2_str = "2222222222222222222222222222222222222222222222222222222222222222";

    let id1 = ContainerId::try_from(id1_str).unwrap();
    let id2 = ContainerId::try_from(id2_str).unwrap();

    let info1 = ContainerInfo {
        id: id1.clone(),
        name: "myapp-web-1".to_string(),
        app: Some("myapp".to_string()),
        service: Some(ServiceName::try_from("web").unwrap()),
        copy: Some(CopyIndex::new(1).unwrap()),
        image: "myapp:latest".to_string(),
        image_id: "sha256:img1".to_string(),
        host_port: None,
        restart: None,
        state: ContainerState::Running,
    };

    let info2 = ContainerInfo {
        id: id2.clone(),
        name: "myapp-web-2".to_string(),
        app: Some("myapp".to_string()),
        service: Some(ServiceName::try_from("web").unwrap()),
        copy: Some(CopyIndex::new(2).unwrap()),
        image: "myapp:latest".to_string(),
        image_id: "sha256:img1".to_string(),
        host_port: None,
        restart: None,
        state: ContainerState::Exited { code: 137 },
    };

    let short_id1 = id1_str[..12].to_string();
    let mut samples = HashMap::new();
    samples.insert(
        short_id1.clone(),
        ContainerStats {
            id: short_id1,
            cpu_percent: 2.5,
            memory_bytes: 1048576,
        },
    );

    let tally = RestartTally::new();
    let rows = container_rows(&[info1, info2], &samples, &tally, &Restarts::new());

    assert_eq!(rows.len(), 2, "must generate exactly two rows");

    let row1 = &rows[0];
    assert_eq!(row1.process_id, "myapp@web.1");
    assert_eq!(row1.kind, "web");
    assert_eq!(row1.ordinal, 1);
    assert_eq!(row1.pid, None);
    assert_eq!(row1.status, ProcessStatus::Running);
    assert_eq!(row1.memory_bytes, 1048576);

    let facts1 = row1
        .container
        .as_ref()
        .expect("row 1 must have container facts");
    assert_eq!(facts1.id, id1_str);
    assert_eq!(facts1.image, "myapp:latest");
    assert_eq!(facts1.state, "running");
    assert_eq!(facts1.cpu_percent, 2.5);
    assert_eq!(facts1.exit_code, None);

    let row2 = &rows[1];
    assert_eq!(row2.process_id, "myapp@web.2");
    assert_eq!(row2.status, ProcessStatus::Crashed);

    let facts2 = row2
        .container
        .as_ref()
        .expect("row 2 must have container facts");
    assert_eq!(facts2.state, "exited");
    assert_eq!(facts2.exit_code, Some(137));
}
