//! Caddy router: one site file per app under `<riku root>/caddy`.
//!
//! `/etc/caddy/Caddyfile` imports `<riku root>/caddy/*.caddy`, and a root
//! systemd path unit reloads Caddy when that directory changes. riku never
//! talks to the Caddy admin API: it listens on localhost with no auth.

mod host;
mod site;

pub use host::{has_import, import_line, SYSTEM_CADDYFILE};
pub use site::{remove_site, render_site, site_path, write_site, HttpsPolicy, SiteRequest};
