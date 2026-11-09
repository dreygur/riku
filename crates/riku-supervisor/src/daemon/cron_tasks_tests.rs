use tempfile::TempDir;

use crate::daemon::Supervisor;

fn supervisor(tmp: &TempDir) -> Supervisor {
    let config_dir = tmp.path().join(".riku").join("workers-enabled");
    std::fs::create_dir_all(&config_dir).unwrap();
    Supervisor::new(config_dir).unwrap()
}

fn entries(lines: &[(&str, &str)]) -> Vec<(String, String)> {
    lines
        .iter()
        .map(|(kind, cmd)| (kind.to_string(), cmd.to_string()))
        .collect()
}

fn commands(supervisor: &Supervisor) -> Vec<String> {
    let mut commands: Vec<String> = supervisor
        .cron_scheduler
        .get_jobs()
        .values()
        .map(|job| job.command.clone())
        .collect();
    commands.sort();
    commands
}

#[test]
fn every_entry_is_scheduled_including_repeated_kinds() {
    let tmp = TempDir::new().unwrap();
    let mut supervisor = supervisor(&tmp);

    supervisor
        .load_cron_jobs(
            "shop",
            &entries(&[("cron", "0 1 * * * ./a"), ("cron", "0 2 * * * ./b")]),
        )
        .unwrap();

    assert_eq!(commands(&supervisor), vec!["./a", "./b"]);
}

#[test]
fn reload_replaces_the_apps_old_jobs() {
    let tmp = TempDir::new().unwrap();
    let mut supervisor = supervisor(&tmp);

    supervisor
        .load_cron_jobs("shop", &entries(&[("cron-a", "0 1 * * * ./a")]))
        .unwrap();
    supervisor
        .load_cron_jobs("shop", &entries(&[("cron-b", "0 3 * * * ./b")]))
        .unwrap();

    assert_eq!(commands(&supervisor), vec!["./b"]);
}
