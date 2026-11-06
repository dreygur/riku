use std::fs;
use tempfile::TempDir;

use super::*;
use crate::config::RikuPaths;

#[test]
fn ps_shows_container_columns() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
    let app = "edge";
    fs::create_dir_all(paths.app_root.join(app)).unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();

    let stats_json = format!(
        r#"[{{"app":"{app}","total_processes":1,"running_processes":1,"healthy_processes":1,
            "total_restarts":0,"total_memory_bytes":1048576,"total_cpu_time_ms":0,
            "processes":[{{"process_id":"{app}-web-1","app":"{app}","kind":"web","ordinal":1,
            "pid":null,"status":"running","started_at":null,"last_health_check":null,
            "health_check_status":"unknown","restart_count":3,"last_restart_at":null,
            "cpu_time_ms":0,"memory_bytes":1048576,"requests_total":0,"requests_per_second":0.0,
            "container":{{"id":"c0ffee1234567890abcdef","image":"ghcr.io/org/repo:v1",
            "state":"running","exit_code":null,"cpu_percent":5.2}}}}],
            "last_updated":"2026-11-02T00:00:00Z"}}]"#,
        app = app
    );
    fs::write(paths.riku_root.join("stats.json"), stats_json).unwrap();

    let (headers, rows) = container_rows_for_app(&paths, app).unwrap();
    assert_eq!(
        headers,
        vec![
            "SERVICE",
            "STATUS",
            "MEMORY",
            "CPU",
            "RESTARTS",
            "CONTAINER ID",
            "IMAGE"
        ]
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0],
        vec![
            "web.1",
            "running",
            "1.0 MB",
            "5.2%",
            "3",
            "c0ffee123456",
            "ghcr.io/org/repo:v1"
        ]
    );
}

#[test]
fn ps_shows_container_exited_status() {
    let tmp = TempDir::new().unwrap();
    let paths = RikuPaths::for_tests(tmp.path());
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
            "container":{{"id":"deadbeef1234","image":"ghcr.io/org/repo:v1",
            "state":"exited","exit_code":137,"cpu_percent":0.0}}}}],
            "last_updated":"2026-11-02T00:00:00Z"}}]"#,
        app = app
    );
    fs::write(paths.riku_root.join("stats.json"), stats_json).unwrap();

    let (_, rows) = container_rows_for_app(&paths, app).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][1], "exited 137");
}
