/// Core performance benchmarks for Riku.
///
/// Covers the hot paths exercised on every deploy and supervisor tick:
/// - Worker config TOML parsing (supervisor reads configs on startup and on change)
/// - Stats JSON serialization (written every supervisor tick to stats.json)
/// - Stats JSON deserialization (read by CLI and health server)
/// - App-level stats aggregation (performed before every metrics response)
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;

// ── TOML fixtures ─────────────────────────────────────────────────────────────

fn minimal_worker_toml() -> &'static str {
    r#"
[worker]
app = "myapp"
kind = "web"
command = "python app.py"
ordinal = 1

[env]
PORT = "5000"

[options]
working_dir = "/home/deploy/.riku/apps/myapp"
log_file = "/home/deploy/.riku/logs/myapp/web.1.log"
"#
}

fn full_worker_toml() -> &'static str {
    r#"
[worker]
app = "myapp"
kind = "web"
command = "gunicorn -w 4 -b 0.0.0.0:5000 app:application"
ordinal = 1

[env]
PORT = "5000"
DATABASE_URL = "postgres://user:pass@localhost/mydb"
REDIS_URL = "redis://localhost:6379/0"
SECRET_KEY = "supersecretkey1234567890abcdef"
DEBUG = "false"
LOG_LEVEL = "info"
WORKERS = "4"
BIND_ADDRESS = "127.0.0.1"

[options]
working_dir = "/home/deploy/.riku/apps/myapp"
log_file = "/home/deploy/.riku/logs/myapp/web.1.log"
timeout = 30
grace_period = 10
max_restarts = 5

[options.health_check]
url = "/health"
interval = 30
timeout = 5
retries = 3
"#
}

// ── Stats JSON fixtures ───────────────────────────────────────────────────────

fn make_process_stat(app: &str, kind: &str, ordinal: u32) -> serde_json::Value {
    serde_json::json!({
        "process_id": format!("{}-{}-{}", app, kind, ordinal),
        "app": app,
        "kind": kind,
        "ordinal": ordinal,
        "pid": 12345u32 + ordinal,
        "status": "Running",
        "started_at": "2026-01-01T00:00:00Z",
        "last_health_check": "2026-01-01T00:01:00Z",
        "health_check_status": "Healthy",
        "restart_count": 0u32,
        "last_restart_at": null,
        "cpu_time_ms": 1500u64,
        "memory_bytes": 52428800u64,
        "requests_total": 10000u64,
        "requests_per_second": 12.5f64
    })
}

fn make_app_stats(app: &str, process_count: u32) -> serde_json::Value {
    let processes: Vec<serde_json::Value> = (1..=process_count)
        .map(|i| make_process_stat(app, "web", i))
        .collect();

    serde_json::json!({
        "app": app,
        "total_processes": process_count,
        "running_processes": process_count,
        "healthy_processes": process_count,
        "total_restarts": 0u32,
        "total_memory_bytes": 52428800u64 * process_count as u64,
        "total_cpu_time_ms": 1500u64 * process_count as u64,
        "processes": processes,
        "last_updated": "2026-01-01T00:01:00Z"
    })
}

// ── Benchmarks ────────────────────────────────────────────────────────────────

fn bench_toml_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("toml_parse");

    group.bench_function("minimal_config", |b| {
        b.iter(|| {
            let _: toml::Value = toml::from_str(black_box(minimal_worker_toml())).unwrap();
        })
    });

    group.bench_function("full_config_with_health_check", |b| {
        b.iter(|| {
            let _: toml::Value = toml::from_str(black_box(full_worker_toml())).unwrap();
        })
    });

    // Simulate supervisor loading N configs at startup
    for n in [1usize, 10, 50] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("batch_configs", n), &n, |b, &n| {
            b.iter(|| {
                for _ in 0..n {
                    let _: toml::Value = toml::from_str(black_box(full_worker_toml())).unwrap();
                }
            })
        });
    }

    group.finish();
}

fn bench_stats_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_serialization");

    // Single app, varying process counts (scales with app worker count)
    for procs in [1u32, 4, 8] {
        let stats = vec![make_app_stats("myapp", procs)];
        let json_val = serde_json::Value::Array(stats);

        group.throughput(Throughput::Elements(procs as u64));
        group.bench_with_input(
            BenchmarkId::new("serialize_single_app", procs),
            &json_val,
            |b, v| b.iter(|| serde_json::to_string(black_box(v)).unwrap()),
        );
    }

    // Multiple apps (scales with number of deployed apps)
    for app_count in [1usize, 5, 20] {
        let stats: Vec<serde_json::Value> = (0..app_count)
            .map(|i| make_app_stats(&format!("app{}", i), 2))
            .collect();
        let json_val = serde_json::Value::Array(stats);

        group.throughput(Throughput::Elements(app_count as u64));
        group.bench_with_input(
            BenchmarkId::new("serialize_multi_app", app_count),
            &json_val,
            |b, v| b.iter(|| serde_json::to_string(black_box(v)).unwrap()),
        );
    }

    group.finish();
}

fn bench_stats_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_deserialization");

    // Pre-serialized JSON strings for deserialization benchmarks
    for app_count in [1usize, 5, 20] {
        let stats: Vec<serde_json::Value> = (0..app_count)
            .map(|i| make_app_stats(&format!("app{}", i), 2))
            .collect();
        let json_str = serde_json::to_string(&stats).unwrap();

        group.throughput(Throughput::Bytes(json_str.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("deserialize_multi_app", app_count),
            &json_str,
            |b, s| {
                b.iter(|| {
                    let _: serde_json::Value = serde_json::from_str(black_box(s)).unwrap();
                })
            },
        );
    }

    group.finish();
}

fn bench_stats_app_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_app_filter");

    // Simulates /metrics/apps/{app} filtering: scan array for matching app name
    for app_count in [5usize, 20, 100] {
        let apps: Vec<serde_json::Value> = (0..app_count)
            .map(|i| make_app_stats(&format!("app{}", i), 2))
            .collect();
        let target = format!("app{}", app_count / 2); // Target middle app (worst-case linear scan)

        group.throughput(Throughput::Elements(app_count as u64));
        group.bench_with_input(
            BenchmarkId::new("filter_by_app_name", app_count),
            &(apps, target),
            |b, (apps, target)| {
                b.iter(|| {
                    apps.iter().find(|a| {
                        a.get("app").and_then(|v| v.as_str()) == Some(black_box(target.as_str()))
                    })
                })
            },
        );
    }

    group.finish();
}

fn bench_env_map_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("env_map");

    // Building env HashMap from a flat list (done for each worker spawn)
    group.bench_function("build_env_map_8_vars", |b| {
        b.iter(|| {
            let mut map = HashMap::new();
            for (k, v) in [
                ("PORT", "5000"),
                ("DATABASE_URL", "postgres://localhost/mydb"),
                ("REDIS_URL", "redis://localhost:6379"),
                ("SECRET_KEY", "abc123"),
                ("DEBUG", "false"),
                ("LOG_LEVEL", "info"),
                ("WORKERS", "4"),
                ("BIND_ADDRESS", "127.0.0.1"),
            ] {
                map.insert(black_box(k.to_string()), black_box(v.to_string()));
            }
            map
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_toml_parse,
    bench_stats_serialization,
    bench_stats_deserialization,
    bench_stats_app_filter,
    bench_env_map_operations,
);
criterion_main!(benches);
