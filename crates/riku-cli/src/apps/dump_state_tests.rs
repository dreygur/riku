use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

use super::supervisor_files::{load_app_stats, supervisor_uptime, supervisor_uptime_seconds};
use super::*;
use crate::config::RikuPaths;

fn make_paths(tmp: &TempDir) -> RikuPaths {
    let paths = RikuPaths::for_tests(tmp.path());
    for dir in &[
        &paths.app_root,
        &paths.env_root,
        &paths.nginx_root,
        &paths.riku_root,
    ] {
        fs::create_dir_all(dir).unwrap();
    }
    paths
}

#[test]
fn test_extract_routing_fields_keeps_only_allowlisted_keys() {
    let mut env = HashMap::new();
    env.insert("PORT".to_string(), "5000".to_string());
    env.insert("DATABASE_URL".to_string(), "postgres://secret".to_string());
    env.insert("SECRET_KEY".to_string(), "abc123".to_string());
    env.insert("RIKU_UPSTREAM_PORT".to_string(), "5000".to_string());

    let routing = extract_routing_fields(&env);

    assert_eq!(routing.len(), 2);
    assert_eq!(routing.get("PORT").map(String::as_str), Some("5000"));
    assert_eq!(
        routing.get("RIKU_UPSTREAM_PORT").map(String::as_str),
        Some("5000")
    );
    assert!(
        !routing.contains_key("DATABASE_URL"),
        "secret env vars must never appear in routing output"
    );
    assert!(
        !routing.contains_key("SECRET_KEY"),
        "secret env vars must never appear in routing output"
    );
}

#[test]
fn test_extract_routing_fields_empty_env_returns_empty_map() {
    assert!(extract_routing_fields(&HashMap::new()).is_empty());
}

#[test]
fn test_build_app_entry_never_leaks_secret_env_vars() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let app = "secretapp";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    let env_dir = paths.env_root.join(app);
    fs::create_dir_all(&env_dir).unwrap();
    fs::write(
        env_dir.join("ENV"),
        "PORT=8080\nDATABASE_URL=postgres://user:pass@host/db\nAPI_TOKEN=topsecret\n",
    )
    .unwrap();

    let entry = build_app_entry(app, &paths, &HashMap::new());
    let serialized = serde_json::to_string(&entry).unwrap();

    assert!(serialized.contains("8080"), "PORT should be present");
    assert!(
        !serialized.contains("postgres"),
        "DATABASE_URL must not appear anywhere in the dump: {}",
        serialized
    );
    assert!(
        !serialized.contains("topsecret"),
        "API_TOKEN must not appear anywhere in the dump: {}",
        serialized
    );
}

#[test]
fn test_build_app_entry_reports_lock_state() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "lockedapp";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();

    let free_entry = build_app_entry(app, &paths, &HashMap::new());
    assert!(matches!(free_entry.deploy_lock, LockState::Free));

    let _held = crate::deploy::lock::acquire(app, &paths).unwrap();
    let held_entry = build_app_entry(app, &paths, &HashMap::new());
    assert!(matches!(held_entry.deploy_lock, LockState::Held));
}

/// An app whose first deploy failed has no release; the dashboard shows it
/// as never deployed instead of stopped.
#[test]
fn released_says_whether_any_deploy_recorded_a_release() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    fs::create_dir_all(paths.app_root.join("shop")).unwrap();
    let released = |paths: &RikuPaths| build_app_entry("shop", paths, &HashMap::new()).released;

    assert!(!released(&paths), "no deploy recorded a release yet");
    crate::deploy::releases::ReleaseLog::new(&paths)
        .record("shop", "0123456789abcdef")
        .unwrap();
    assert!(released(&paths), "a recorded release counts");
}

fn enable_worker(paths: &RikuPaths, app: &str, kind: &str) {
    fs::create_dir_all(&paths.workers_enabled).unwrap();
    let config = crate::supervisor::config::create_worker_config(
        app,
        kind,
        "run",
        1,
        HashMap::new(),
        "/srv/app",
        "/srv/log",
    );
    fs::write(
        paths.workers_enabled.join(format!("{app}@{kind}.1.toml")),
        toml::to_string(&config).unwrap(),
    )
    .unwrap();
}

/// `urls` says where the app is live, as the push summary does: a port
/// mode app on a LAN address gets its URL labelled `lan`, while a worker
/// only app and an app that never deployed get none.
#[test]
fn urls_are_the_push_summary_urls_and_none_without_a_route() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    crate::config::settings::save_key(&paths, "routing", "server_ip", Some("10.0.0.5")).unwrap();
    let state = paths.riku_root.join("routing");
    fs::create_dir_all(&state).unwrap();
    fs::write(
        state.join("state.toml"),
        "[apps.qs-py]\nmode = \"port\"\nport = 8102\n",
    )
    .unwrap();
    let releases = crate::deploy::releases::ReleaseLog::new(&paths);
    for (app, kind) in [("qs-py", "web"), ("jobs", "worker"), ("fresh", "web")] {
        fs::create_dir_all(paths.app_root.join(app)).unwrap();
        fs::write(
            paths.app_root.join(app).join("riku.toml"),
            "[routing]\nmode = \"port\"\nport = 8102\n",
        )
        .unwrap();
        enable_worker(&paths, app, kind);
        if app != "fresh" {
            releases.record(app, "0123456789abcdef").unwrap();
        }
    }
    let urls = |app: &str| {
        serde_json::to_value(build_app_entry(app, &paths, &HashMap::new())).unwrap()["urls"].clone()
    };

    assert_eq!(
        urls("qs-py"),
        serde_json::json!([{ "url": "http://10.0.0.5:8102", "label": "lan" }])
    );
    assert_eq!(urls("jobs"), serde_json::json!([]), "worker only app");
    assert_eq!(urls("fresh"), serde_json::json!([]), "never deployed app");
}

/// The dashboard shows one scale control per kind the scale API takes: the
/// long running kinds of SCALING (a kind scaled to 0 included) and of the
/// workers, never a one shot or cron kind, and no `web` the app lacks.
#[test]
fn process_kinds_are_the_scalable_kinds_of_scaling_and_workers() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "limittest";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();
    fs::write(
        paths.env_root.join(app).join("SCALING"),
        "release=1\ncron1=1\nworker=1\nmailer=0\n",
    )
    .unwrap();

    let entry = build_app_entry(app, &paths, &HashMap::new());

    assert_eq!(entry.process_kinds, vec!["mailer", "worker"]);
}

/// The router helper writes `/etc/riku/nginx/<app>.conf`; a config left in
/// the deploy user's own `nginx_root` is not one nginx loads.
#[test]
fn nginx_state_reads_the_site_the_helper_writes() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let site_root = tmp.path().join("etc-riku-nginx");
    fs::create_dir_all(&site_root).unwrap();
    let state = || {
        serde_json::to_value(route::nginx_site_state(
            &site_root.join("webapp.conf"),
            &paths,
        ))
        .unwrap()
    };

    fs::write(paths.nginx_root.join("webapp.conf"), "# stray").unwrap();
    assert_eq!(state()["config_exists"], false, "{}", state());
    assert_eq!(state()["config_known"], true);

    fs::write(site_root.join("webapp.conf"), "# conf").unwrap();
    assert_eq!(state()["router"], "nginx");
    assert_eq!(state()["config_exists"], true, "{}", state());
}

/// Like a Caddy site, a site root this user cannot search says nothing
/// about the config, so it reads as unknown, not missing.
#[test]
fn nginx_state_is_unknown_when_the_site_root_is_closed() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let site_root = tmp.path().join("closed");
    fs::create_dir_all(&site_root).unwrap();
    fs::set_permissions(&site_root, fs::Permissions::from_mode(0o000)).unwrap();

    let state = route::nginx_site_state(&site_root.join("webapp.conf"), &paths);
    fs::set_permissions(&site_root, fs::Permissions::from_mode(0o755)).unwrap();

    if nix::unistd::Uid::effective().is_root() {
        return; // root searches any directory, so there is nothing to observe
    }
    assert_eq!(
        state,
        route::RouteState::Nginx {
            config_exists: false,
            config_known: false,
            enabled: false,
        }
    );
}

#[test]
fn test_build_app_entry_includes_workers_from_stats() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "workedapp";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();

    let stats_json = format!(
        r#"[{{"app":"{app}","total_processes":1,"running_processes":1,"healthy_processes":1,
                "total_restarts":0,"total_memory_bytes":0,"total_cpu_time_ms":0,
                "processes":[{{"process_id":"{app}-web-1","app":"{app}","kind":"web","ordinal":1,
                "pid":1234,"status":"running","started_at":null,"last_health_check":null,
                "health_check_status":"unknown","restart_count":0,"last_restart_at":null,
                "cpu_time_ms":0,"memory_bytes":0,"requests_total":0,"requests_per_second":0.0}}],
                "last_updated":"2024-01-01T00:00:00Z"}}]"#,
        app = app
    );
    fs::write(paths.riku_root.join("stats.json"), stats_json).unwrap();

    let app_stats = load_app_stats(&paths);
    let entry = build_app_entry(app, &paths, &app_stats);

    assert_eq!(entry.workers.len(), 1);
    assert_eq!(entry.workers[0].pid, Some(1234));
    assert_eq!(entry.workers[0].process_id, format!("{}-web-1", app));
}

#[test]
fn test_load_app_stats_missing_file_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    assert!(load_app_stats(&paths).is_empty());
}

#[test]
fn test_supervisor_uptime_seconds_missing_pid_file_returns_none() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    assert!(supervisor_uptime_seconds(&paths).is_none());
}

/// The pid file outlives its supervisor, so only a locked one has an uptime.
#[test]
fn supervisor_uptime_needs_a_locked_pid_file() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let pid_file = paths.riku_root.join("supervisor.pid");
    fs::write(&pid_file, "1\n").unwrap();
    assert_eq!(supervisor_uptime_seconds(&paths), None, "unlocked: exited");

    let held = fs::File::open(&pid_file).unwrap();
    held.lock().unwrap();
    assert!(supervisor_uptime_seconds(&paths).is_some());
}

/// Root never reads the deploy user's pid file, so it must say why the
/// uptime is missing instead of looking like a stopped supervisor.
#[test]
fn root_reports_the_uptime_as_unavailable() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let pid_file = paths.riku_root.join("supervisor.pid");
    fs::write(&pid_file, "1\n").unwrap();
    let held = fs::File::open(&pid_file).unwrap();
    held.lock().unwrap();

    let as_root = supervisor_uptime(&paths, true);
    assert_eq!(as_root.seconds, None, "root must not read the pid file");
    assert!(
        as_root
            .unavailable
            .is_some_and(|why| why.contains("as root")),
        "root gets a reason: {:?}",
        as_root.unavailable
    );

    let as_user = supervisor_uptime(&paths, false);
    assert!(
        as_user.seconds.is_some(),
        "the deploy user reads the uptime"
    );
    assert_eq!(as_user.unavailable, None);
}

#[test]
fn test_build_state_dump_end_to_end_no_panic() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "fullapp";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();
    fs::write(
        paths.env_root.join(app).join("ENV"),
        "PORT=3000\nSECRET=hide\n",
    )
    .unwrap();

    let dump = build_state_dump(&paths).unwrap();
    let serialized = serde_json::to_string(&dump).unwrap();

    assert_eq!(dump.apps.len(), 1);
    assert!(serialized.contains("3000"));
    assert!(!serialized.contains("hide"));
}

#[test]
fn dump_state_reports_caddy_site() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "shop";
    fs::create_dir_all(paths.env_root.join(app)).unwrap();
    crate::config::settings::save_key(&paths, "router", "name", Some("caddy")).unwrap();
    fs::create_dir_all(&paths.caddy_root).unwrap();
    let site = paths.caddy_root.join("shop.caddy");
    fs::write(&site, "# riku app: shop\n").unwrap();

    let route = serde_json::to_value(build_app_entry(app, &paths, &HashMap::new()).route).unwrap();
    assert_eq!(
        route,
        serde_json::json!({ "router": "caddy", "site": site.display().to_string(), "site_known": true }),
        "caddy route should carry the site path and no nginx fields"
    );
}

#[test]
fn caddy_site_is_unknown_not_missing_when_the_site_root_is_unreadable() {
    use std::os::unix::fs::PermissionsExt;
    if nix::unistd::Uid::effective().is_root() {
        return;
    }
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "shop";
    fs::create_dir_all(paths.env_root.join(app)).unwrap();
    crate::config::settings::save_key(&paths, "router", "name", Some("caddy")).unwrap();
    fs::create_dir_all(&paths.caddy_root).unwrap();
    fs::write(paths.caddy_root.join("shop.caddy"), "# riku app: shop\n").unwrap();
    fs::set_permissions(&paths.caddy_root, fs::Permissions::from_mode(0o000)).unwrap();

    let route = serde_json::to_value(build_app_entry(app, &paths, &HashMap::new()).route).unwrap();

    fs::set_permissions(&paths.caddy_root, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        route,
        serde_json::json!({ "router": "caddy", "site": null, "site_known": false }),
        "a root-only site root must read as unknown, not as no site"
    );
}

#[test]
fn container_row_has_engine_fields() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "edge";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();

    let stats_json = format!(
        r#"[{{"app":"{app}","total_processes":1,"running_processes":1,"healthy_processes":1,
            "total_restarts":0,"total_memory_bytes":1024,"total_cpu_time_ms":0,
            "processes":[{{"process_id":"{app}-web-1","app":"{app}","kind":"web","ordinal":1,
            "pid":null,"status":"running","started_at":null,"last_health_check":null,
            "health_check_status":"unknown","restart_count":2,"last_restart_at":null,
            "cpu_time_ms":0,"memory_bytes":1024,"requests_total":0,"requests_per_second":0.0,
            "container":{{"id":"c0ffee1234567890abcdef","image":"ghcr.io/org/repo:tag",
            "state":"running","exit_code":null,"cpu_percent":4.5}}}}],
            "last_updated":"2026-11-02T00:00:00Z"}}]"#,
        app = app
    );
    fs::write(paths.riku_root.join("stats.json"), stats_json).unwrap();

    let app_stats = load_app_stats(&paths);
    let entry = build_app_entry(app, &paths, &app_stats);

    assert_eq!(entry.workers.len(), 1);
    let worker = &entry.workers[0];
    assert_eq!(worker.unit, UnitKind::Container);
    assert_eq!(worker.container_id, Some("c0ffee123456".to_string()));
    assert_eq!(worker.image, Some("ghcr.io/org/repo:tag".to_string()));
    assert_eq!(worker.state_detail, Some("running".to_string()));
    assert_eq!(worker.cpu_percent, Some(4.5));

    let val = serde_json::to_value(worker).unwrap();
    assert_eq!(val["unit"], "container");
    assert_eq!(val["container_id"], "c0ffee123456");
    assert_eq!(val["image"], "ghcr.io/org/repo:tag");
    assert_eq!(val["state_detail"], "running");
    assert_eq!(val["cpu_percent"], 4.5);
}

#[test]
fn container_row_exited_shows_exit_code() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "edge";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();

    let stats_json = format!(
        r#"[{{"app":"{app}","total_processes":1,"running_processes":0,"healthy_processes":0,
            "total_restarts":1,"total_memory_bytes":0,"total_cpu_time_ms":0,
            "processes":[{{"process_id":"{app}-web-1","app":"{app}","kind":"web","ordinal":1,
            "pid":null,"status":"crashed","started_at":null,"last_health_check":null,
            "health_check_status":"unknown","restart_count":1,"last_restart_at":null,
            "cpu_time_ms":0,"memory_bytes":0,"requests_total":0,"requests_per_second":0.0,
            "container":{{"id":"deadbeef123456","image":"edge:latest",
            "state":"exited","exit_code":137,"cpu_percent":0.0}}}}],
            "last_updated":"2026-11-02T00:00:00Z"}}]"#,
        app = app
    );
    fs::write(paths.riku_root.join("stats.json"), stats_json).unwrap();

    let app_stats = load_app_stats(&paths);
    let entry = build_app_entry(app, &paths, &app_stats);

    assert_eq!(entry.workers.len(), 1);
    let worker = &entry.workers[0];
    assert_eq!(worker.state_detail, Some("exited 137".to_string()));
}

#[test]
fn process_row_unchanged() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let app = "myapp";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();

    let stats_json = format!(
        r#"[{{"app":"{app}","total_processes":1,"running_processes":1,"healthy_processes":1,
            "total_restarts":0,"total_memory_bytes":1000,"total_cpu_time_ms":200,
            "processes":[{{"process_id":"{app}-web-1","app":"{app}","kind":"web","ordinal":1,
            "pid":4242,"status":"running","started_at":null,"last_health_check":null,
            "health_check_status":"unknown","restart_count":0,"last_restart_at":null,
            "cpu_time_ms":200,"memory_bytes":1000,"memory_limit_bytes":268435456,
            "requests_total":0,"requests_per_second":0.0}}],
            "last_updated":"2026-11-02T00:00:00Z"}}]"#,
        app = app
    );
    fs::write(paths.riku_root.join("stats.json"), stats_json).unwrap();

    let app_stats = load_app_stats(&paths);
    let entry = build_app_entry(app, &paths, &app_stats);

    assert_eq!(entry.workers.len(), 1);
    let worker = &entry.workers[0];
    assert_eq!(worker.unit, UnitKind::Process);
    assert_eq!(worker.container_id, None);
    assert_eq!(worker.image, None);
    assert_eq!(worker.state_detail, None);
    assert_eq!(worker.cpu_percent, None);

    let val = serde_json::to_value(worker).unwrap();
    let expected = serde_json::json!({
        "process_id": format!("{app}-web-1"),
        "kind": "web",
        "ordinal": 1,
        "pid": 4242,
        "status": "running",
        "restart_count": 0,
        "memory_bytes": 1000,
        "memory_limit_bytes": 268435456,
        "cpu_time_ms": 200,
        "unit": "process",
        "container_id": null,
        "image": null,
        "state_detail": null,
        "cpu_percent": null
    });
    assert_eq!(val, expected);
}

#[test]
fn engine_missing_is_null() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    crate::config::settings::save_key(&paths, "engine", "program", Some("missing-engine")).unwrap();
    let dump = build_state_dump(&paths).unwrap();
    assert_eq!(dump.engine, None);

    let val = serde_json::to_value(&dump).unwrap();
    assert!(val.get("engine").is_some());
    assert!(val["engine"].is_null());
}

#[test]
fn engine_present_reports_info() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let engine_bin = tmp.path().join("mock-engine");
    let script = "#!/bin/sh\n\
        case \"$2\" in --help) exit 0;; esac\n\
        case \"$1\" in version) echo '{\"Server\":{\"Platform\":{\"Name\":\"Podman\"},\"Version\":\"5.0.0\"}}'; exit 0;; esac\n\
        exit 0\n";
    fs::write(&engine_bin, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&engine_bin, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let engine = engine_bin.display().to_string();
    crate::config::settings::save_key(&paths, "engine", "program", Some(&engine)).unwrap();

    let dump = build_state_dump(&paths).unwrap();
    assert_eq!(
        dump.engine,
        Some(EngineInfo {
            name: "Podman".to_string(),
            version: "5.0.0".to_string(),
        })
    );
    let val = serde_json::to_value(&dump).unwrap();
    assert_eq!(val["engine"]["name"], "Podman");
    assert_eq!(val["engine"]["version"], "5.0.0");
}
