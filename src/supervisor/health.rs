//! Health check and metrics HTTP endpoint for monitoring and load balancers.
//!
//! Provides a simple HTTP server that responds to:
//! - /health             - Supervisor health status in JSON
//! - /metrics            - All process metrics from stats.json
//! - /metrics/apps       - Per-app aggregated metrics
//! - /metrics/apps/{app} - Metrics for a specific app

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use serde_json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};
use threadpool::ThreadPool;

/// Start the health check HTTP server on the specified port.
///
/// The server runs in a background thread and responds to:
/// - GET /health - JSON containing status, uptime, version
/// - GET /metrics - JSON metrics from stats.json
///
/// # Arguments
/// * `port` - Port to bind to (default: 9091)
/// * `running` - Atomic flag to signal shutdown
/// * `start_time` - When the supervisor started
/// * `stats_file` - Path to stats.json file
///
/// # Returns
/// * `Ok(())` if server started successfully
/// * `Err` if failed to bind to port
pub fn start_health_server(
    port: u16,
    running: Arc<AtomicBool>,
    start_time: SystemTime,
    stats_file: PathBuf,
) -> anyhow::Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)?;
    listener.set_nonblocking(true)?;

    tracing::info!(
        "Health server listening on http://{} (/health, /metrics, /metrics/apps, /metrics/apps/{{app}})",
        addr
    );

    thread::spawn(move || {
        // Use thread pool to handle concurrent requests (max 4 concurrent)
        // This prevents DoS attacks from blocking health checks
        let pool = ThreadPool::new(4);

        while running.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let start_time_clone = start_time;
                    let stats_file_clone = stats_file.clone();

                    // Handle request in thread pool
                    pool.execute(move || {
                        if let Err(e) = handle_request(stream, start_time_clone, &stats_file_clone)
                        {
                            tracing::warn!("Health server request failed: {}", e);
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No connection available, sleep briefly
                    thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    tracing::error!("Health server error: {}", e);
                    break;
                }
            }
        }

        // Wait for all pending requests to complete before shutdown
        pool.join();
        tracing::info!("Health server stopped");
    });

    Ok(())
}

/// Handle a single HTTP request
fn handle_request(
    mut stream: TcpStream,
    start_time: SystemTime,
    stats_file: &PathBuf,
) -> anyhow::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    // Read request (we only care about the first line)
    let mut buffer = [0u8; 1024];
    let n = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..n]);

    // Parse request line
    let first_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    if parts.len() >= 2 && parts[0] == "GET" {
        let path = parts[1];
        if path == "/health" {
            send_health_response(&mut stream, start_time)?;
        } else if path == "/metrics" {
            send_metrics_response(&mut stream, stats_file)?;
        } else if path == "/metrics/apps" {
            send_app_metrics_response(&mut stream, stats_file, None)?;
        } else if let Some(app) = path.strip_prefix("/metrics/apps/") {
            if app.is_empty() {
                send_app_metrics_response(&mut stream, stats_file, None)?;
            } else {
                send_app_metrics_response(&mut stream, stats_file, Some(app))?;
            }
        } else {
            send_404_response(&mut stream)?;
        }
    } else {
        send_404_response(&mut stream)?;
    }

    Ok(())
}

/// Send health check response
fn send_health_response(stream: &mut TcpStream, start_time: SystemTime) -> anyhow::Result<()> {
    let uptime = start_time
        .elapsed()
        .unwrap_or(Duration::from_secs(0))
        .as_secs();

    let version = env!("CARGO_PKG_VERSION");

    let json = format!(
        r#"{{"status":"healthy","uptime":{},"version":"{}","timestamp":{}}}"#,
        uptime,
        version,
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
    );

    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        json.len(),
        json
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok(())
}

/// Send metrics response (stats.json content)
fn send_metrics_response(stream: &mut TcpStream, stats_file: &PathBuf) -> anyhow::Result<()> {
    let json = if stats_file.exists() {
        fs::read_to_string(stats_file)
            .unwrap_or_else(|_| r#"{"error":"Failed to read stats"}"#.to_string())
    } else {
        r#"{"error":"Stats not available yet"}"#.to_string()
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        json.len(),
        json
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok(())
}

/// Send per-app metrics response.
///
/// If `app` is `Some`, filters to just that app. If `None`, returns all apps.
/// Returns 404 JSON if a specific app is not found.
fn send_app_metrics_response(
    stream: &mut TcpStream,
    stats_file: &PathBuf,
    app: Option<&str>,
) -> anyhow::Result<()> {
    let raw = if stats_file.exists() {
        fs::read_to_string(stats_file)
            .unwrap_or_else(|_| "[]".to_string())
    } else {
        "[]".to_string()
    };

    // Parse the stats array and filter if needed
    let json = match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(serde_json::Value::Array(apps)) => {
            if let Some(app_name) = app {
                // Find the specific app
                let found = apps
                    .iter()
                    .find(|a| a.get("app").and_then(|v| v.as_str()) == Some(app_name));
                match found {
                    Some(v) => serde_json::to_string(v)
                        .unwrap_or_else(|_| r#"{"error":"Serialization failed"}"#.to_string()),
                    None => {
                        let body = format!(
                            r#"{{"error":"Not Found","app":"{}","message":"No metrics for this app"}}"#,
                            app_name
                        );
                        let response = format!(
                            "HTTP/1.1 404 Not Found\r\n\
                             Content-Type: application/json\r\n\
                             Content-Length: {}\r\n\
                             Connection: close\r\n\
                             \r\n\
                             {}",
                            body.len(),
                            body
                        );
                        stream.write_all(response.as_bytes())?;
                        stream.flush()?;
                        return Ok(());
                    }
                }
            } else {
                raw
            }
        }
        _ => raw,
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        json.len(),
        json
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok(())
}

/// Send 404 response
fn send_404_response(stream: &mut TcpStream) -> anyhow::Result<()> {
    let body = r#"{"error":"Not Found","message":"Try GET /health or GET /metrics"}"#;
    let response = format!(
        "HTTP/1.1 404 Not Found\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        body.len(),
        body
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
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
        assert!(!body.contains("otherapp"), "Should only return myapp, not otherapp");

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
}
