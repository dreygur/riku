//! HTTP response helpers for the health check server.

use std::fs;
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Send health check response
pub(super) fn send_health_response(
    stream: &mut TcpStream,
    start_time: SystemTime,
) -> anyhow::Result<()> {
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
pub(super) fn send_metrics_response(
    stream: &mut TcpStream,
    stats_file: &PathBuf,
) -> anyhow::Result<()> {
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
pub(super) fn send_app_metrics_response(
    stream: &mut TcpStream,
    stats_file: &PathBuf,
    app: Option<&str>,
) -> anyhow::Result<()> {
    let raw = if stats_file.exists() {
        fs::read_to_string(stats_file).unwrap_or_else(|_| "[]".to_string())
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
pub(super) fn send_404_response(stream: &mut TcpStream) -> anyhow::Result<()> {
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
