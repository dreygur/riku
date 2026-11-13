use tempfile::TempDir;

use crate::config::create_worker_config;
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

fn write_config(supervisor: &Supervisor, file: &str, kind: &str, env: &[(&str, &str)]) {
    let env = env
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    let config = create_worker_config("shop", kind, "run", 1, env, "/srv/shop", "/srv/log");
    std::fs::write(
        supervisor.config_dir.join(file),
        toml::to_string(&config).unwrap(),
    )
    .unwrap();
}

#[test]
fn cron_jobs_take_the_cron_config_env_over_a_web_copy_that_sorts_first() {
    let tmp = TempDir::new().unwrap();
    let supervisor = supervisor(&tmp);
    write_config(
        &supervisor,
        "shop-web-1.toml",
        "web",
        &[("DATABASE_URL", "db"), ("PORT", "5000")],
    );
    write_config(
        &supervisor,
        "shop@cron.1.toml",
        "cron",
        &[("DATABASE_URL", "db")],
    );

    let (_, env) = supervisor.get_app_context("shop");

    assert_eq!(
        env.get("PORT"),
        None,
        "cron got the web copy's env instead of the cron config's: {env:?}"
    );
    assert_eq!(env.get("DATABASE_URL").map(String::as_str), Some("db"));
}

#[test]
fn cron_jobs_fall_back_to_the_first_config_without_a_cron_config() {
    let tmp = TempDir::new().unwrap();
    let supervisor = supervisor(&tmp);
    write_config(&supervisor, "shop@web.1.toml", "web", &[("PORT", "5000")]);
    write_config(&supervisor, "shop@worker.1.toml", "worker", &[]);

    let (dir, env) = supervisor.get_app_context("shop");

    assert_eq!(dir.as_deref(), Some("/srv/shop"));
    assert_eq!(env.get("PORT").map(String::as_str), Some("5000"));
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
