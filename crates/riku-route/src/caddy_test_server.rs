//! A Caddy-only router helper that runs inside a test process.
//!
//! Tests of the deploy side need something on the helper's socket. This
//! serves the real [`apply_caddy`] and [`remove_caddy`] over a socket in the
//! test's own directory, with the mock `caddy` the test installed in
//! `paths.reload_programs`. It is public because other crates' tests call it
//! and must never be started by production code: it answers anyone who can
//! reach its socket. The `test-server` feature gates it, and only the
//! dev-dependencies of crates with such tests turn that feature on.

use std::fs;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;

use nix::unistd::Uid;
use riku_config::RikuPaths;

use crate::answer::answer;
use crate::caddy_apply::CaddyRoots;
use crate::nginx_roots::{NginxRoots, ReloadMethod};
use crate::server::serve_one;

/// Nginx roots that exist nowhere, so a stray nginx request fails instead of
/// touching the host.
fn unused_nginx_roots(owner_uid: u32) -> NginxRoots {
    let none = |name: &str| PathBuf::from("/nonexistent/riku-test").join(name);
    NginxRoots {
        site_root: none("sites"),
        cert_root: none("certs"),
        socket_root: none("sockets"),
        static_root: none("static"),
        acme_root: none("acme"),
        nginx: none("nginx"),
        reload: ReloadMethod::Signal,
        ipv6: false,
        owner_uid,
    }
}

/// Point `paths.router_socket` at a new socket next to `paths.caddy_root`,
/// create the site directory and serve Caddy requests on the socket from a
/// background thread that ends with the test process.
pub fn serve_caddy_for_tests(paths: &mut RikuPaths) {
    let owner = Uid::current().as_raw();
    let roots = CaddyRoots {
        site_root: paths.caddy_root.clone(),
        caddy: paths.reload_programs.caddy.clone(),
        caddyfile: paths.caddy_root.with_file_name("Caddyfile"),
        admin_address: "unix//test/admin.sock".to_string(),
        owner_uid: owner,
    };
    let nginx = unused_nginx_roots(owner);
    paths.router_socket = paths.caddy_root.with_file_name("router.sock");
    fs::create_dir_all(&roots.site_root).expect("create the test Caddy site directory");
    let listener = UnixListener::bind(&paths.router_socket).expect("bind the test router socket");
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = serve_one(stream, owner, &mut |request| {
                answer(&nginx, &roots, request)
            });
        }
    });
}
