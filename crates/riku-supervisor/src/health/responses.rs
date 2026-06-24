//! Pure data-building functions for HTTP responses.
//!
//! These functions produce JSON data without performing any I/O.
//! The Axum handlers in `mod.rs` wrap the results in `Json(...)` or error responses.

use std::path::Path;
use std::time::{Duration, SystemTime};

/// Build the health check JSON value.
///
/// Returns `{"status":"healthy","uptime":<secs>,"version":"<ver>","timestamp":<epoch>}`.
pub(super) fn build_health_json(start_time: SystemTime) -> serde_json::Value {
    let uptime = start_time
        .elapsed()
        .unwrap_or(Duration::from_secs(0))
        .as_secs();

    let version = env!("CARGO_PKG_VERSION");

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs();

    serde_json::json!({
        "status": "healthy",
        "uptime": uptime,
        "version": version,
        "timestamp": timestamp
    })
}

/// Read the raw metrics JSON string from the stats file.
///
/// Returns the file contents as a string, or a JSON error object if the file
/// does not exist or cannot be read.
pub(super) fn build_metrics_json(stats_file: &Path) -> String {
    if stats_file.exists() {
        std::fs::read_to_string(stats_file)
            .unwrap_or_else(|_| r#"{"error":"Failed to read stats"}"#.to_string())
    } else {
        r#"{"error":"Stats not available yet"}"#.to_string()
    }
}

/// Build per-app metrics JSON string.
///
/// If `app` is `Some(name)`, filters to that specific app and returns its
/// object. If `None`, returns the full array. Returns a JSON error object
/// if the requested app is not found.
pub(super) fn build_app_metrics_json(stats_file: &Path, app: Option<&str>) -> String {
    let raw = if stats_file.exists() {
        std::fs::read_to_string(stats_file).unwrap_or_else(|_| "[]".to_string())
    } else {
        "[]".to_string()
    };

    let parsed = serde_json::from_str::<serde_json::Value>(&raw);

    match parsed {
        Ok(serde_json::Value::Array(apps)) => {
            if let Some(app_name) = app {
                let found = apps
                    .iter()
                    .find(|a| a.get("app").and_then(|v| v.as_str()) == Some(app_name));
                match found {
                    Some(v) => serde_json::to_string(v)
                        .unwrap_or_else(|_| r#"{"error":"Serialization failed"}"#.to_string()),
                    None => format!(
                        r#"{{"error":"Not Found","app":"{}","message":"No metrics for this app"}}"#,
                        app_name
                    ),
                }
            } else {
                raw
            }
        }
        _ => raw,
    }
}
