//! Fixed paths outside the deploy user's home that root init creates and
//! the router helper, nginx or Caddy use.

/// Where uWSGI copies put their unix sockets on a real host. It sits outside
/// the deploy user's home so nginx needs no access to that home. Root init
/// creates it owned by the deploy user, group [`SOCKET_GROUP`], mode `2750`.
pub const SOCKET_ROOT: &str = "/run/riku-sockets";

/// The group shared by the deploy user and the nginx worker user, and the
/// only group that can reach [`SOCKET_ROOT`].
pub const SOCKET_GROUP: &str = "riku-sockets";

/// The router helper's socket. The helper binds it itself, directly in `/run`
/// so the deploy user needs no access to any directory below it.
pub const ROUTER_SOCKET: &str = "/run/riku-router.sock";

/// Exists on a host that booted with systemd as init.
pub const SYSTEMD_RUNTIME_DIR: &str = "/run/systemd/system";

/// Where Caddy site files live on a real host. It sits outside the deploy
/// user's home so the `caddy` user needs no access to that home. Root init
/// creates it owned by root, group `caddy`, mode `2750`; only the router
/// helper writes site files here.
pub const CADDY_SITE_ROOT: &str = "/etc/riku/caddy";

/// Root-owned `0755` directory of the nginx site configs the router helper
/// renders, one `<app>.conf` each. The host include loads `*.conf` from here.
pub const NGINX_SITE_ROOT: &str = "/etc/riku/nginx";

/// Root-owned `0700` directory of per app TLS material, `<app>/fullchain.pem`
/// and `<app>/key.pem`, written by the router helper.
pub const CERT_ROOT: &str = "/etc/riku/certs";

/// Root-owned `0755` parent of the directories nginx serves files from.
pub const VAR_ROOT: &str = "/var/lib/riku";

/// The ACME http-01 webroot on a real host. Root init creates it owned by
/// the deploy user, group [`SOCKET_GROUP`], mode `2750`, so the nginx worker
/// reads challenge files without any access to the deploy home.
pub const ACME_WEBROOT: &str = "/var/lib/riku/acme";

/// Where deploys publish each app's static files for nginx, one directory
/// per app. Same owner, group and mode as [`ACME_WEBROOT`].
pub const STATIC_ROOT: &str = "/var/lib/riku/static";
