//! Health check and metrics HTTP endpoint for monitoring and load balancers.
//!
//! Provides a simple HTTP server that responds to:
//! - /health             - Supervisor health status in JSON
//! - /metrics            - All process metrics from stats.json
//! - /metrics/apps       - Per-app aggregated metrics
//! - /metrics/apps/{app} - Metrics for a specific app

mod responses;

#[cfg(test)]
mod tests;

use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};
use threadpool::ThreadPool;

use responses::{send_404_response, send_app_metrics_response, send_health_response, send_metrics_response};

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

