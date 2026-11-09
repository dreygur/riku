use std::collections::HashMap;

use tempfile::TempDir;

use crate::daemon::Supervisor;

fn supervisor(tmp: &TempDir) -> Supervisor {
    let config_dir = tmp.path().join(".riku").join("workers-enabled");
    std::fs::create_dir_all(&config_dir).unwrap();
    Supervisor::new(config_dir).unwrap()
}

#[test]
fn riku_toml_cron_job_is_scheduled() {
    let tmp = TempDir::new().unwrap();
    let mut supervisor = supervisor(&tmp);
    let processes = HashMap::from([
        ("web".to_string(), "node server.js".to_string()),
        (
            "cron-backup".to_string(),
            "0 2 * * * ./backup.sh".to_string(),
        ),
    ]);

    supervisor.load_cron_jobs("shop", &processes).unwrap();

    let commands: Vec<&str> = supervisor
        .cron_scheduler
        .get_jobs()
        .values()
        .map(|job| job.command.as_str())
        .collect();
    assert_eq!(commands, vec!["./backup.sh"], "jobs: {commands:?}");
}

#[test]
fn reload_replaces_the_apps_old_jobs() {
    let tmp = TempDir::new().unwrap();
    let mut supervisor = supervisor(&tmp);
    let first = HashMap::from([("cron-a".to_string(), "0 1 * * * ./a".to_string())]);
    let second = HashMap::from([("cron-b".to_string(), "0 3 * * * ./b".to_string())]);

    supervisor.load_cron_jobs("shop", &first).unwrap();
    supervisor.load_cron_jobs("shop", &second).unwrap();

    let commands: Vec<&str> = supervisor
        .cron_scheduler
        .get_jobs()
        .values()
        .map(|job| job.command.as_str())
        .collect();
    assert_eq!(commands, vec!["./b"]);
}
