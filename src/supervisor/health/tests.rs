use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use super::*;

fn start_test_server(
    port: u16,
    running: Arc<AtomicBool>,
    stats_file: std::path::PathBuf,
) -> broadcast::Sender<String> {
    let start_time = SystemTime::now();
    // Derive the control token path from the test's own temp dir (stats_file's
    // parent) rather than the real `$HOME/.riku/`, so tests never read or
    // write the developer's actual control token file.
    let control_token_file = stats_file
        .parent()
        .expect("stats_file must have a parent dir")
        .join("control.token");
    start_health_server(port, running, start_time, stats_file, control_token_file)
        .expect("failed to start test health server")
}

fn wait_for_server(port: u16) {
    for _ in 0..50 {
        if std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("server on port {} did not start in time", port);
}

#[test]
fn test_health_endpoint() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    let running = Arc::new(AtomicBool::new(true));
    let port = 19101;

    let _tx = start_test_server(port, running.clone(), stats_file);
    wait_for_server(port);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://127.0.0.1:{}/health", port))
        .send()
        .expect("request failed");

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().expect("invalid JSON");
    assert_eq!(body["status"], "healthy");
    assert!(body["version"].is_string());
    assert!(body["uptime"].is_number());
    assert!(body["timestamp"].is_number());

    running.store(false, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(500));
}

#[test]
fn test_metrics_endpoint_empty() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    let running = Arc::new(AtomicBool::new(true));
    let port = 19102;

    let _tx = start_test_server(port, running.clone(), stats_file);
    wait_for_server(port);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://127.0.0.1:{}/metrics", port))
        .send()
        .expect("request failed");

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().expect("invalid JSON");
    assert!(body.get("error").is_some());

    running.store(false, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(500));
}

#[test]
fn test_metrics_apps_endpoint_empty() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    fs::write(&stats_file, "[]").unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let port = 19103;

    let _tx = start_test_server(port, running.clone(), stats_file);
    wait_for_server(port);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://127.0.0.1:{}/metrics/apps", port))
        .send()
        .expect("request failed");

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().expect("invalid JSON");
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 0);

    running.store(false, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(500));
}

#[test]
fn test_metrics_apps_endpoint_with_data() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    let stats = r#"[{"app":"myapp","total_processes":2,"running_processes":2,"healthy_processes":1,"total_restarts":0,"total_memory_bytes":0,"total_cpu_time_ms":0,"processes":[],"last_updated":"2026-01-01T00:00:00Z"}]"#;
    fs::write(&stats_file, stats).unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let port = 19104;

    let _tx = start_test_server(port, running.clone(), stats_file);
    wait_for_server(port);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://127.0.0.1:{}/metrics/apps", port))
        .send()
        .expect("request failed");

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().expect("invalid JSON");
    assert!(body.is_array());
    let apps = body.as_array().unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0]["app"], "myapp");

    running.store(false, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(500));
}

#[test]
fn test_metrics_app_specific_found() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    let stats = r#"[{"app":"myapp","total_processes":1,"running_processes":1,"healthy_processes":1,"total_restarts":0,"total_memory_bytes":0,"total_cpu_time_ms":0,"processes":[],"last_updated":"2026-01-01T00:00:00Z"},{"app":"otherapp","total_processes":1,"running_processes":0,"healthy_processes":0,"total_restarts":0,"total_memory_bytes":0,"total_cpu_time_ms":0,"processes":[],"last_updated":"2026-01-01T00:00:00Z"}]"#;
    fs::write(&stats_file, stats).unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let port = 19105;

    let _tx = start_test_server(port, running.clone(), stats_file);
    wait_for_server(port);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://127.0.0.1:{}/metrics/apps/myapp", port))
        .send()
        .expect("request failed");

    let status = resp.status();
    let body_text = resp.text().unwrap_or_default();

    assert_eq!(status, 200, "expected 200, got body: {}", body_text);

    let body: serde_json::Value =
        serde_json::from_str(&body_text).expect("response is not valid JSON");
    assert_eq!(body["app"], "myapp");
    assert!(body.get("total_processes").is_some());

    running.store(false, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(500));
}

#[test]
fn test_metrics_app_specific_not_found() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    fs::write(&stats_file, "[]").unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let port = 19106;

    let _tx = start_test_server(port, running.clone(), stats_file);
    wait_for_server(port);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/metrics/apps/nonexistent",
            port
        ))
        .send()
        .expect("request failed");

    let status = resp.status();
    let body_text = resp.text().unwrap_or_default();

    assert_eq!(status, 404, "expected 404, got body: {}", body_text);

    let body: serde_json::Value =
        serde_json::from_str(&body_text).expect("response is not valid JSON");
    assert_eq!(body["error"], "Not Found");

    running.store(false, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(500));
}

#[test]
fn test_404_response() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    let running = Arc::new(AtomicBool::new(true));
    let port = 19107;

    let _tx = start_test_server(port, running.clone(), stats_file);
    wait_for_server(port);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://127.0.0.1:{}/invalid", port))
        .send()
        .expect("request failed");

    assert_eq!(resp.status(), 404);

    running.store(false, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(500));
}

#[test]
fn test_metrics_stream_returns_sse_headers() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    fs::write(&stats_file, r#"[]"#).unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let port = 19108;

    let _tx = start_test_server(port, running.clone(), stats_file);
    wait_for_server(port);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://127.0.0.1:{}/metrics/stream", port))
        .send()
        .expect("request failed");

    assert_eq!(resp.status(), 200);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(
        content_type.contains("text/event-stream"),
        "Expected SSE content type, got: {}",
        content_type
    );

    running.store(false, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(500));
}
