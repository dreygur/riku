//! The dashboard UI: `index.html`, its stylesheet and its ES modules, all
//! embedded into the binary at compile time from `../assets/`.
//!
//! Security model: a request path is only ever used as a key into the fixed
//! `ASSETS` table, never joined onto a filesystem path, so there is nothing to
//! traverse. Like the index, the assets are public: they carry no data, and a
//! browser loading `<script type="module">` or a stylesheet cannot attach the
//! Bearer token. The data behind them stays gated in `routes::authorize`.

use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};

const INDEX_HTML: &str = include_str!("../assets/index.html");

const CSS: &str = "text/css; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";

/// One `ASSETS` entry, served at `/assets/<path>` from `assets/<path>`.
macro_rules! asset {
    ($path:literal, $content_type:expr) => {
        (
            $path,
            $content_type,
            include_str!(concat!("../assets/", $path)),
        )
    };
}

/// Every file served under `/assets/`, as (path, content type, body).
pub(crate) const ASSETS: &[(&str, &str, &str)] = &[
    asset!("app.css", CSS),
    asset!("js/actions.js", JS),
    asset!("js/api.js", JS),
    asset!("js/auth.js", JS),
    asset!("js/card-events.js", JS),
    asset!("js/card.js", JS),
    asset!("js/dom.js", JS),
    asset!("js/format.js", JS),
    asset!("js/icons.js", JS),
    asset!("js/logs/drawer.js", JS),
    asset!("js/logs/stream.js", JS),
    asset!("js/main.js", JS),
    asset!("js/page.js", JS),
    asset!("js/poll.js", JS),
    asset!("js/releases.js", JS),
    asset!("js/render/apps.js", JS),
    asset!("js/render/containers.js", JS),
    asset!("js/render/meta.js", JS),
    asset!("js/render/panels.js", JS),
    asset!("js/render/workers.js", JS),
    asset!("js/scale.js", JS),
    asset!("js/state.js", JS),
    asset!("js/toast.js", JS),
];

pub(crate) async fn index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

/// GET /assets/{*path}: one entry of `ASSETS`, or 404.
pub(crate) async fn asset(Path(path): Path<String>) -> Response {
    match ASSETS.iter().find(|(name, _, _)| *name == path) {
        Some((_, content_type, body)) => {
            ([(header::CONTENT_TYPE, *content_type)], *body).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
