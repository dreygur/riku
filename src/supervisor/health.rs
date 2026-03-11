//! Health check and metrics HTTP endpoint for monitoring and load balancers.
//!
//! Provides a simple HTTP server that responds to:
//! - /health - Supervisor health status in JSON
//! - /metrics - Process metrics from stats.json

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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
        "Health server listening on http://{} (/health, /metrics)",
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
        match parts[1] {
            "/health" => send_health_response(&mut stream, start_time)?,
            "/metrics" => send_metrics_response(&mut stream, stats_file)?,
            _ => send_404_response(&mut stream)?,
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
}
