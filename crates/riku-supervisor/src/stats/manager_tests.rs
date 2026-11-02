use super::StatsManager;
use crate::stats::types::{ContainerFacts, ProcessStats, ProcessStatus};

#[test]
fn container_rows_replace_only_container_rows() {
    let mut manager = StatsManager::new();
    manager.register_process(
        "myapp-web-1".to_string(),
        "myapp".to_string(),
        "web".to_string(),
        1,
    );

    let old_container = ProcessStats {
        process_id: "myapp-worker-1".to_string(),
        app: "myapp".to_string(),
        kind: "worker".to_string(),
        ordinal: 1,
        status: ProcessStatus::Running,
        container: Some(ContainerFacts {
            id: "old-id".to_string(),
            image: "myapp:old".to_string(),
            state: "running".to_string(),
            exit_code: None,
            cpu_percent: 1.0,
        }),
        ..Default::default()
    };
    manager.replace_container_rows(vec![old_container]);

    let new_container = ProcessStats {
        process_id: "myapp-worker-2".to_string(),
        app: "myapp".to_string(),
        kind: "worker".to_string(),
        ordinal: 2,
        status: ProcessStatus::Running,
        container: Some(ContainerFacts {
            id: "new-id".to_string(),
            image: "myapp:new".to_string(),
            state: "running".to_string(),
            exit_code: None,
            cpu_percent: 2.0,
        }),
        ..Default::default()
    };
    manager.replace_container_rows(vec![new_container]);

    assert!(
        manager.stats.contains_key("myapp-web-1"),
        "Procfile process must remain untouched when container rows are replaced"
    );
    assert!(
        !manager.stats.contains_key("myapp-worker-1"),
        "previous container row must be removed"
    );
    assert!(
        manager.stats.contains_key("myapp-worker-2"),
        "new container row must be inserted"
    );
}

#[test]
fn reset_restarts_clears_count() {
    let mut manager = StatsManager::new();
    manager.register_process(
        "myapp-web-1".to_string(),
        "myapp".to_string(),
        "web".to_string(),
        1,
    );
    manager.mark_restarting("myapp-web-1");

    let before = manager
        .stats
        .get("myapp-web-1")
        .expect("process must exist");
    assert_eq!(before.restart_count, 1);
    assert!(before.last_restart_at.is_some());

    manager.reset_restarts("myapp-web-1");

    let after = manager
        .stats
        .get("myapp-web-1")
        .expect("process must exist");
    assert_eq!(
        after.restart_count, 0,
        "restart_count must be 0 after reset"
    );
    assert!(
        after.last_restart_at.is_none(),
        "last_restart_at must be None after reset"
    );
}
