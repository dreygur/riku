//! `riku __dump-state` — read-only export of the supervisor's in-memory
//! state matrix (port allocations, worker PIDs, deploy locks, nginx
//! routing) as structured JSON, for operator visibility without
//! interrupting the running supervisor.
//!
//! # Security model
//!
//! Everything here is read from on-disk state the supervisor itself
//! already persists (`stats.json`, app `ENV` files, nginx configs, deploy
//! lock files) — this command never connects to the running supervisor
//! process, so it can't disturb it, and it never blocks: the deploy-lock
//! check is a non-blocking probe (see `deploy::lock::is_locked`) that's
//! released immediately if it happens to succeed.
//!
//! App `ENV` files routinely hold customer secrets (`DATABASE_URL`,
//! `SECRET_KEY`, API tokens, ...). Those must never appear in this dump.
//! [`ROUTING_ENV_ALLOWLIST`] is a strict allowlist, not a blocklist: only
//! the handful of known, non-secret routing/port keys are ever read out of
//! an app's env map. Everything else is silently omitted, never just
//! masked — there's no value in printing `SECRET_KEY: "***"` and pretending
//! that's safe against a future key that looks like a secret but isn't on
//! some hypothetical blocklist.

use anyhow::Result;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use crate::config::RikuPaths;
use crate::deploy::lock;
use crate::supervisor::stats::{AppStats, ProcessStatus};

/// Known non-secret routing/port keys an app's `ENV` file may carry.
/// Strict allowlist — see module docs for why this isn't a blocklist.
const ROUTING_ENV_ALLOWLIST: &[&str] = &[
    "PORT",
    "SOCKET",
    "UWSGI_SOCKET",
    "NGINX_INTERNAL_PORT",
    "NGINX_EXTERNAL_PORT",
    "NGINX_PORTMAP",
    "NGINX_WSGI",
    "NGINX_HTTPS_ONLY",
    "NGINX_SERVER_NAME",
    "RUNTIME",
];

#[derive(Serialize)]
struct StateDump {
    generated_at: u64,
    riku_version: &'static str,
    supervisor_uptime_seconds: Option<u64>,
    apps: Vec<AppStateEntry>,
}

#[derive(Serialize)]
struct AppStateEntry {
    app: String,
    deploy_lock: LockState,
    /// Allowlisted routing/port env vars only — see module docs.
    routing: BTreeMap<String, String>,
    nginx: NginxState,
    workers: Vec<WorkerStateEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum LockState {
    Free,
    Held,
}

#[derive(Serialize)]
struct NginxState {
    config_exists: bool,
    enabled: bool,
}

#[derive(Serialize)]
struct WorkerStateEntry {
    process_id: String,
    kind: String,
    ordinal: u32,
    pid: Option<u32>,
    status: ProcessStatus,
    restart_count: u32,
}

/// Dump the current supervisor state matrix as pretty-printed JSON to stdout.
pub fn cmd_dump_state(paths: &RikuPaths) -> Result<()> {
    let dump = build_state_dump(paths)?;
    println!("{}", serde_json::to_string_pretty(&dump)?);
    Ok(())
}

fn build_state_dump(paths: &RikuPaths) -> Result<StateDump> {
    let app_stats = load_app_stats(paths);

    let mut apps = Vec::new();
    if paths.app_root.exists() {
        for entry in fs::read_dir(&paths.app_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(app) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            apps.push(build_app_entry(&app, paths, &app_stats));
        }
    }
    apps.sort_by(|a, b| a.app.cmp(&b.app));

    Ok(StateDump {
        generated_at: unix_timestamp_now(),
        riku_version: env!("CARGO_PKG_VERSION"),
        supervisor_uptime_seconds: supervisor_uptime_seconds(paths),
        apps,
    })
}

fn build_app_entry(
    app: &str,
    paths: &RikuPaths,
    app_stats: &HashMap<String, AppStats>,
) -> AppStateEntry {
    let env_file = paths.env_root.join(app).join("ENV");
    let mut env_map = HashMap::new();
    let _ = crate::util::parse_settings(&env_file, &mut env_map);

    let nginx_config = paths.nginx_root.join(format!("{}.conf", app));
    let nginx_enabled_symlink = Path::new("/etc/nginx/sites-enabled").join(format!("{}.conf", app));

    let workers = app_stats
        .get(app)
        .map(|stats| {
            stats
                .processes
                .iter()
                .map(|p| WorkerStateEntry {
                    process_id: p.process_id.clone(),
                    kind: p.kind.clone(),
                    ordinal: p.ordinal,
                    pid: p.pid,
                    status: p.status.clone(),
                    restart_count: p.restart_count,
                })
                .collect()
        })
        .unwrap_or_default();

    AppStateEntry {
        app: app.to_string(),
        deploy_lock: if lock::is_locked(app, paths) {
            LockState::Held
        } else {
            LockState::Free
        },
        routing: extract_routing_fields(&env_map),
        nginx: NginxState {
            config_exists: nginx_config.exists(),
            enabled: nginx_enabled_symlink.exists(),
        },
        workers,
    }
}

/// Filter `env` down to the allowlisted routing/port keys only. See module
/// docs for why this is an allowlist, not a mask-and-keep-the-rest.
fn extract_routing_fields(env: &HashMap<String, String>) -> BTreeMap<String, String> {
    ROUTING_ENV_ALLOWLIST
        .iter()
        .filter_map(|&key| env.get(key).map(|v| (key.to_string(), v.clone())))
        .collect()
}

/// Load `stats.json` (written periodically by the running supervisor) as
/// `app -> AppStats`, or an empty map if it doesn't exist yet / fails to
/// parse — a dump with no worker data is still useful for inspecting
/// routing and lock state, so this never errors the whole command out.
fn load_app_stats(paths: &RikuPaths) -> HashMap<String, AppStats> {
    let stats_file = paths.riku_root.join("stats.json");
    let Ok(content) = fs::read_to_string(&stats_file) else {
        return HashMap::new();
    };
    let Ok(stats_vec) = serde_json::from_str::<Vec<AppStats>>(&content) else {
        return HashMap::new();
    };
    stats_vec.into_iter().map(|s| (s.app.clone(), s)).collect()
}

/// Approximate supervisor uptime from the PID file's mtime — written once,
/// at startup, by `create_pid_file_with_lock`. This command never connects
/// to the running supervisor process, so this is the closest available
/// proxy for "when did it start" without one.
fn supervisor_uptime_seconds(paths: &RikuPaths) -> Option<u64> {
    let pid_file = paths.riku_root.join("supervisor.pid");
    let modified = fs::metadata(&pid_file).ok()?.modified().ok()?;
    SystemTime::now()
        .duration_since(modified)
        .ok()
        .map(|d| d.as_secs())
}

fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_paths(tmp: &TempDir) -> RikuPaths {
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
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
        env.insert("NGINX_INTERNAL_PORT".to_string(), "5000".to_string());

        let routing = extract_routing_fields(&env);

        assert_eq!(routing.len(), 2);
        assert_eq!(routing.get("PORT").map(String::as_str), Some("5000"));
        assert_eq!(
            routing.get("NGINX_INTERNAL_PORT").map(String::as_str),
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

    #[test]
    fn test_build_app_entry_nginx_state() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let app = "webapp";
        fs::create_dir_all(paths.app_root.join(app)).unwrap();
        fs::create_dir_all(paths.env_root.join(app)).unwrap();

        let before = build_app_entry(app, &paths, &HashMap::new());
        assert!(!before.nginx.config_exists);

        fs::write(paths.nginx_root.join(format!("{}.conf", app)), "# conf").unwrap();
        let after = build_app_entry(app, &paths, &HashMap::new());
        assert!(after.nginx.config_exists);
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

    #[test]
    fn test_supervisor_uptime_seconds_present() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        fs::write(paths.riku_root.join("supervisor.pid"), "1\n").unwrap();
        let uptime = supervisor_uptime_seconds(&paths);
        assert!(uptime.is_some());
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
}
