//! Live log streaming over Server-Sent Events.
//!
//! Tails every `*.log` under `{log_root}/{app}/` (deploy log + per-worker
//! stdout/stderr) and pushes new lines to the browser as they're written. Each
//! file is followed from its current end, polled on a short interval; a client
//! disconnect drops the channel and the tailer task exits on its next send.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_stream::wrappers::ReceiverStream;

use super::routes::authorize;
use super::DashboardState;

#[derive(Deserialize)]
pub(crate) struct LogQuery {
    token: Option<String>,
}

/// GET /api/apps/:app/logs — SSE stream of the app's combined logs.
pub(crate) async fn stream(
    State(state): State<DashboardState>,
    headers: HeaderMap,
    Path(app): Path<String>,
    Query(query): Query<LogQuery>,
) -> Response {
    // Reuse the read-only API gate (token / loopback Host) — EventSource cannot
    // set headers, so the token may arrive as ?token=.
    let mut q = HashMap::new();
    if let Some(t) = query.token {
        q.insert("token".to_string(), t);
    }
    if let Some(denied) = authorize(&state, &headers, &q) {
        return denied;
    }
    let app = match crate::util::validate_app_name(&app) {
        Ok(a) => a,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };

    let dir = state.paths.log_root.join(&app);
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, std::convert::Infallible>>(256);

    tokio::spawn(async move {
        // Per-file read offset; start each file at its current end so we stream
        // only new output, not the whole history.
        let mut offsets: HashMap<PathBuf, u64> = HashMap::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("log") {
                    let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    offsets.insert(p, len);
                }
            }
        }
        let _ = tx.send(Ok(Event::default().comment("streaming"))).await;

        loop {
            // Discover newly created log files too.
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("log") {
                        offsets.entry(p).or_insert(0);
                    }
                }
            }

            for (path, offset) in offsets.iter_mut() {
                let Ok(mut f) = tokio::fs::File::open(&path).await else {
                    continue;
                };
                let len = f.metadata().await.map(|m| m.len()).unwrap_or(*offset);
                if len < *offset {
                    *offset = 0; // file truncated/rotated — restart from the top
                }
                if len > *offset {
                    if f.seek(std::io::SeekFrom::Start(*offset)).await.is_err() {
                        continue;
                    }
                    let mut buf = String::new();
                    if f.read_to_string(&mut buf).await.is_ok() {
                        *offset = len;
                        let tag = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("log")
                            .to_string();
                        for line in buf.lines() {
                            if tx
                                .send(Ok(Event::default()
                                    .event("log")
                                    .data(format!("{tag}\t{line}"))))
                                .await
                                .is_err()
                            {
                                return; // client gone
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(800)).await;
        }
    });

    Sse::new(ReceiverStream::new(rx))
        .keep_alive(axum::response::sse::KeepAlive::default())
        .into_response()
}
