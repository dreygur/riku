use riku_supervisor::stats::{AppStats, ContainerFacts, ProcessStats, ProcessStatus};

use super::running_containers;

fn copy(app: &str, status: ProcessStatus) -> ProcessStats {
    ProcessStats {
        app: app.to_string(),
        status,
        container: Some(ContainerFacts {
            id: "0123456789ab".to_string(),
            image: "redis:7-alpine".to_string(),
            state: "running".to_string(),
            exit_code: None,
            cpu_percent: 0.0,
        }),
        ..ProcessStats::default()
    }
}

fn app_stats(app: &str, processes: Vec<ProcessStats>) -> AppStats {
    AppStats {
        app: app.to_string(),
        total_processes: processes.len() as u32,
        running_processes: 0,
        healthy_processes: 0,
        total_restarts: 0,
        total_memory_bytes: 0,
        total_cpu_time_ms: 0,
        processes,
        last_updated: chrono::Utc::now(),
    }
}

#[test]
fn a_compose_app_counts_its_running_containers() {
    let stats = vec![
        app_stats(
            "ct-compose",
            vec![
                copy("ct-compose", ProcessStatus::Running),
                copy("ct-compose", ProcessStatus::Running),
                copy("ct-compose", ProcessStatus::Crashed),
                ProcessStats {
                    app: "ct-compose".to_string(),
                    status: ProcessStatus::Running,
                    ..ProcessStats::default()
                },
            ],
        ),
        app_stats("other", vec![copy("other", ProcessStatus::Running)]),
    ];

    assert_eq!(running_containers(Some(&stats), "ct-compose"), 2);
    assert_eq!(running_containers(Some(&stats), "missing"), 0);
    assert_eq!(running_containers(None, "ct-compose"), 0);
}
