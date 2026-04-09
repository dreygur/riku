use super::*;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

#[test]
fn test_health_endpoint() {
    use tempfile::TempDir;
    let running = Arc::new(AtomicBool::new(true));
    let start_time = SystemTime::now();
    let temp_dir = TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");

    // Start server on random port
    let port = 19091; // Test port
    start_health_server(port, running.clone(), start_time, stats_file).unwrap();

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    // Test health endpoint
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(response.contains("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""status":"healthy""#));
    assert!(response.contains(r#""version":"#));
    assert!(response.contains(r#""uptime":"#));

    // Stop server
    running.store(false, Ordering::Relaxed);
}

#[test]
fn test_404_response() {
    use tempfile::TempDir;
    let running = Arc::new(AtomicBool::new(true));
    let start_time = SystemTime::now();
    let temp_dir = TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    let port = 19092; // Different port

    start_health_server(port, running.clone(), start_time, stats_file).unwrap();
    thread::sleep(Duration::from_millis(100));

    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream
        .write_all(b"GET /invalid HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(response.contains("HTTP/1.1 404 Not Found"));
    assert!(response.contains(r#""error":"Not Found""#));

    running.store(false, Ordering::Relaxed);
}

#[test]
fn test_metrics_apps_endpoint_empty() {
    use tempfile::TempDir;
    let running = Arc::new(AtomicBool::new(true));
    let start_time = SystemTime::now();
    let temp_dir = TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    // Write empty stats array
    fs::write(&stats_file, "[]").unwrap();
    let port = 19093;

    start_health_server(port, running.clone(), start_time, stats_file).unwrap();
    thread::sleep(Duration::from_millis(100));

    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream
        .write_all(b"GET /metrics/apps HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(response.contains("HTTP/1.1 200 OK"));
    assert!(response.contains("[]"));

    running.store(false, Ordering::Relaxed);
}

#[test]
fn test_metrics_apps_endpoint_with_data() {
    use tempfile::TempDir;
    let running = Arc::new(AtomicBool::new(true));
    let start_time = SystemTime::now();
    let temp_dir = TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    // Write sample stats
    let stats = r#"[{"app":"myapp","total_processes":2,"running_processes":2,"healthy_processes":1,"total_restarts":0,"total_memory_bytes":0,"total_cpu_time_ms":0,"processes":[],"last_updated":"2026-01-01T00:00:00Z"}]"#;
    fs::write(&stats_file, stats).unwrap();
    let port = 19094;

    start_health_server(port, running.clone(), start_time, stats_file).unwrap();
    thread::sleep(Duration::from_millis(100));

    // /metrics/apps returns all apps
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream
        .write_all(b"GET /metrics/apps HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.contains("HTTP/1.1 200 OK"));
    assert!(response.contains("myapp"));

    running.store(false, Ordering::Relaxed);
}

#[test]
fn test_metrics_app_specific_found() {
    use tempfile::TempDir;
    let running = Arc::new(AtomicBool::new(true));
    let start_time = SystemTime::now();
    let temp_dir = TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    let stats = r#"[{"app":"myapp","total_processes":1,"running_processes":1,"healthy_processes":1,"total_restarts":0,"total_memory_bytes":0,"total_cpu_time_ms":0,"processes":[],"last_updated":"2026-01-01T00:00:00Z"},{"app":"otherapp","total_processes":1,"running_processes":0,"healthy_processes":0,"total_restarts":0,"total_memory_bytes":0,"total_cpu_time_ms":0,"processes":[],"last_updated":"2026-01-01T00:00:00Z"}]"#;
    fs::write(&stats_file, stats).unwrap();
    let port = 19095;

    start_health_server(port, running.clone(), start_time, stats_file).unwrap();
    thread::sleep(Duration::from_millis(100));

    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream
        .write_all(b"GET /metrics/apps/myapp HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(response.contains("HTTP/1.1 200 OK"));
    assert!(response.contains("myapp"));
    // Should not include otherapp in the body
    let body_start = response.find("\r\n\r\n").unwrap_or(0) + 4;
    let body = &response[body_start..];
    assert!(
        !body.contains("otherapp"),
        "Should only return myapp, not otherapp"
    );

    running.store(false, Ordering::Relaxed);
}

#[test]
fn test_metrics_app_specific_not_found() {
    use tempfile::TempDir;
    let running = Arc::new(AtomicBool::new(true));
    let start_time = SystemTime::now();
    let temp_dir = TempDir::new().unwrap();
    let stats_file = temp_dir.path().join("stats.json");
    fs::write(&stats_file, "[]").unwrap();
    let port = 19096;

    start_health_server(port, running.clone(), start_time, stats_file).unwrap();
    thread::sleep(Duration::from_millis(100));

    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream
        .write_all(b"GET /metrics/apps/nonexistent HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(response.contains("HTTP/1.1 404 Not Found"));
    assert!(response.contains("nonexistent"));

    running.store(false, Ordering::Relaxed);
}
