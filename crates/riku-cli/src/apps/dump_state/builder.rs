//! Builders for constructing supervisor state dumps.

use std::collections::{BTreeMap, HashMap};
use std::fs;

use anyhow::Result;
use riku_supervisor::stats::{AppStats, ContainerFacts, ProcessStatus};
use riku_util::file_names::worker::WorkerName;

use crate::config::RikuPaths;
use crate::deploy::lock;
use crate::deploy::releases::ReleaseLog;

use super::kinds::process_kinds;
use super::route::route_state;
use super::supervisor_files::{load_app_stats, supervisor_uptime, unix_timestamp_now};
use super::types::{AppStateEntry, EngineInfo, LockState, StateDump, UnitKind, WorkerStateEntry};

/// Known non-secret routing/port keys an app's `ENV` file may carry.
const ROUTING_ENV_ALLOWLIST: &[&str] = &[
    "PORT",
    "SOCKET",
    "UWSGI_SOCKET",
    "RIKU_UPSTREAM_PORT",
    "RIKU_ROUTING_MODE",
    "RIKU_ROUTING_PORT",
    "RIKU_NGINX_PORTMAP",
    "RIKU_NGINX_WSGI",
    "RIKU_HTTPS_ONLY",
    "RIKU_DOMAINS",
    "RIKU_RUNTIME",
];

fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

fn format_state_detail(facts: &ContainerFacts) -> String {
    match facts.exit_code {
        Some(code) => format!("{} {code}", facts.state),
        None => facts.state.clone(),
    }
}

fn detect_engine(paths: &RikuPaths) -> Option<EngineInfo> {
    match crate::deploy::containers::open_engine(paths) {
        Ok(engine) => {
            let id = engine.identity();
            Some(EngineInfo {
                name: id.name.clone(),
                version: id.version.clone(),
            })
        }
        Err(_) => None,
    }
}

/// Whether the worker's config is still enabled.
fn worker_config_exists(paths: &RikuPaths, app: &str, kind: &str, ordinal: u32) -> bool {
    paths
        .workers_enabled
        .join(WorkerName::new(app, kind, ordinal).file_name())
        .exists()
}

/// Filter `env` down to allowlisted routing/port keys only.
pub fn extract_routing_fields(env: &HashMap<String, String>) -> BTreeMap<String, String> {
    ROUTING_ENV_ALLOWLIST
        .iter()
        .filter_map(|&key| env.get(key).map(|v| (key.to_string(), v.clone())))
        .collect()
}

fn build_worker_entry(p: &crate::supervisor::stats::ProcessStats) -> WorkerStateEntry {
    match &p.container {
        Some(facts) => WorkerStateEntry {
            process_id: p.process_id.clone(),
            kind: p.kind.clone(),
            ordinal: p.ordinal,
            pid: p.pid,
            status: p.status.clone(),
            restart_count: p.restart_count,
            memory_bytes: p.memory_bytes,
            memory_limit_bytes: None,
            cpu_time_ms: p.cpu_time_ms,
            unit: UnitKind::Container,
            container_id: Some(short_id(&facts.id)),
            image: Some(facts.image.clone()),
            state_detail: Some(format_state_detail(facts)),
            cpu_percent: Some(facts.cpu_percent),
        },
        None => WorkerStateEntry {
            process_id: p.process_id.clone(),
            kind: p.kind.clone(),
            ordinal: p.ordinal,
            pid: p.pid,
            status: p.status.clone(),
            restart_count: p.restart_count,
            memory_bytes: p.memory_bytes,
            memory_limit_bytes: p.memory_limit_bytes,
            cpu_time_ms: p.cpu_time_ms,
            unit: UnitKind::Process,
            container_id: None,
            image: None,
            state_detail: None,
            cpu_percent: None,
        },
    }
}

pub fn build_app_entry(
    app: &str,
    paths: &RikuPaths,
    app_stats: &HashMap<String, AppStats>,
) -> AppStateEntry {
    let (env_map, config_error) = match crate::config::app_config::load_env(paths, app) {
        Ok(loaded) => (loaded.env, None),
        Err(e) => (HashMap::new(), Some(format!("{e:#}"))),
    };

    let workers = match app_stats.get(app) {
        Some(stats) => stats
            .processes
            .iter()
            .filter(|p| {
                p.container.is_some()
                    || p.status == ProcessStatus::Running
                    || worker_config_exists(paths, app, &p.kind, p.ordinal)
            })
            .map(build_worker_entry)
            .collect(),
        None => Vec::new(),
    };
    let containers = workers.iter().any(|w| w.unit == UnitKind::Container);
    let process_kinds = if containers {
        Vec::new()
    } else {
        process_kinds(paths, app, &workers)
    };
    let released = !ReleaseLog::new(paths).list(app).is_empty();
    let urls = if released && config_error.is_none() {
        let container =
            containers || crate::deploy::containers::runs_containers(app, paths).unwrap_or(false);
        crate::deploy::app_urls(app, &env_map, paths, container)
    } else {
        crate::deploy::AppUrls::default()
    };

    AppStateEntry {
        app: app.to_string(),
        deploy_lock: if lock::is_locked(app, paths) {
            LockState::Held
        } else {
            LockState::Free
        },
        routing: extract_routing_fields(&env_map),
        route: route_state(app, paths),
        urls: urls.urls,
        pending_urls: urls.pending,
        workers,
        released,
        process_kinds,
        config_error,
    }
}

#[allow(clippy::question_mark)]
pub fn build_state_dump(paths: &RikuPaths) -> Result<StateDump> {
    let app_stats = load_app_stats(paths);
    let mut apps = Vec::new();
    if paths.app_root.exists() {
        let entries = match fs::read_dir(&paths.app_root) {
            Ok(entries) => entries,
            Err(e) => return Err(e.into()),
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => return Err(e.into()),
            };
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(e) => return Err(e.into()),
            };
            if !file_type.is_dir() {
                continue;
            }
            let Some(app) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            apps.push(build_app_entry(&app, paths, &app_stats));
        }
    }
    apps.sort_by(|a, b| a.app.cmp(&b.app));

    let uptime = supervisor_uptime(paths, nix::unistd::Uid::effective().is_root());
    Ok(StateDump {
        generated_at: unix_timestamp_now(),
        riku_version: env!("CARGO_PKG_VERSION"),
        supervisor_uptime_seconds: uptime.seconds,
        supervisor_uptime_unavailable: uptime.unavailable,
        engine: detect_engine(paths),
        apps,
    })
}
