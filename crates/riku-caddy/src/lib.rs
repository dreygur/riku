//! Caddy router: one site file per app under `/etc/riku/caddy`.
//!
//! `/etc/caddy/Caddyfile` imports `/etc/riku/caddy/*.caddy`. The router
//! helper writes the site files and reloads Caddy through its admin socket;
//! the deploy user writes nothing here.

mod host;
mod site;

pub use host::{has_import, import_line, ADMIN_ADDRESS, SYSTEM_CADDYFILE};
pub use riku_util::route_plan::HttpsPolicy;
pub use site::{render_site, site_path};
