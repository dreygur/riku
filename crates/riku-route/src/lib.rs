//! The route the router helper accepts for one app, and the only gate a route
//! passes before it becomes router config.
//!
//! Security model: every field of a [`Route`] is untrusted. The deploy user
//! builds it, sends it to a root process, and that process renders it into
//! files the router parses as root. Nothing here trusts the sender:
//!
//! - Unknown fields are refused at every level, so a field a newer client
//!   knows and this build does not never passes silently.
//! - No field is a path. Paths the rendered config names (sockets, static
//!   copies, certificates) are built by the renderer from the
//!   checked app name and riku-config constants. TLS material travels as PEM
//!   text, never as a file name.
//! - Text that lands in a config file is checked by [`check_nginx_route`]: domains
//!   by the [`riku_util::domain::Domain`] rules, locations against the nginx
//!   directive breaking characters.
//! - TCP backends must be loopback, so a route cannot point the router at
//!   another host.
//! - A TLS pair must be certificates only in one field and exactly one
//!   private key in the other, and the key must belong to the first
//!   certificate, so a key never lands in a readable certificate file. The
//!   gate hands back PEM re-encoded from the parsed DER ([`CheckedRoute`]),
//!   never the request text.
//! - The plain HTTP port is 80 or unprivileged, since the root master binds it.
//!
//! [`check_nginx_route`] and [`check_caddy_route`] are the single gate. The
//! helper calls one again on receipt whatever the sender already did; the
//! deploy side calls only [`check_caddy_route`] for an early error. [`apply_nginx`] and [`remove_nginx`] are the nginx half of the
//! helper and [`apply_caddy`] and [`remove_caddy`] the Caddy half: they call
//! the gate themselves, then render, check and reload as root.

mod answer;
mod bounded;
mod caddy_apply;
#[cfg(feature = "test-server")]
mod caddy_test_server;
mod check;
mod check_text;
mod check_tls;
mod listen;
mod nginx_apply;
mod nginx_render;
mod nginx_roots;
mod nginx_run;
#[cfg(test)]
mod nginx_syntax_tests;
mod paths_apply;
mod paths_check;
mod port_wait;
mod protocol;
mod schema;
mod server;
mod site_files;
#[cfg(test)]
mod test_pems;

pub use answer::answer;
pub use bounded::{is_closed, read_bounded, MAX_MESSAGE_BYTES};
pub use caddy_apply::{apply_caddy, remove_caddy, CaddyRoots};
#[cfg(feature = "test-server")]
pub use caddy_test_server::serve_caddy_for_tests;
pub use check::{check_caddy_route, check_nginx_route, CheckedRoute};
pub use check_tls::check_tls;
pub use listen::{bind_socket, restrict_umask, run};
pub use nginx_apply::{apply_nginx, remove_nginx};
pub use nginx_roots::{NginxRoots, ReloadMethod};
pub use protocol::{
    encode_reply, encode_request, parse_reply, parse_request, Reply, Request, RouterKind, Stage,
    PROTOCOL_VERSION,
};
pub use schema::{Listen, PathApp, Route, RouteBackend, SiteKind, Tls};
pub use server::{serve_one, REQUEST_BUDGET};
