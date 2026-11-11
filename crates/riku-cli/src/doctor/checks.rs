//! Individual diagnostic checks for `riku doctor`.
//!
//! Every function is **read-only**: it inspects the host (PATH, filesystem,
//! `systemctl` status) and reports, but never mutates Riku state.
//! Each returns one or more [`Check`] results carrying a remediation hint on
//! anything that is not [`Status::Ok`].

use std::process::Command;

use nix::sys::statvfs::statvfs;

use crate::config::router::Router;
use crate::config::RikuPaths;

use super::Check;

/// External tools Riku relies on: git is required, systemd is strongly
/// recommended. The router binary is checked by [`super::router_check`].
pub fn dependencies() -> Vec<Check> {
    let mut out = Vec::new();

    if which::which("git").is_ok() {
        out.push(Check::ok("git", "found on PATH"));
    } else {
        out.push(Check::fail(
            "git",
            "not found: git is required (apt install git)",
        ));
    }

    if which::which("systemctl").is_ok() {
        out.push(Check::ok("systemd", "systemctl available"));
    } else {
        out.push(Check::warn(
            "systemd",
            "systemctl not found: run the supervisor manually: riku supervisor",
        ));
    }

    out
}

/// Verify the `~/.riku/` directory tree exists. A missing tree almost always
/// means `riku init` has not been run. The router directory follows the
/// `ROUTER` file; a read error there is reported by the router check, so
/// this one falls back to the nginx directory.
pub fn directories(paths: &RikuPaths) -> Check {
    let router_dir = match Router::load(paths) {
        Ok(Router::Caddy) => ("caddy", paths.caddy_root.as_path()),
        _ => ("nginx", paths.nginx_root.as_path()),
    };
    let dirs: [(&str, &std::path::Path); 8] = [
        ("apps", &paths.app_root),
        ("data", &paths.data_root),
        ("envs", &paths.env_root),
        ("repos", &paths.git_root),
        ("logs", &paths.log_root),
        router_dir,
        ("plugins", &paths.plugin_root),
        ("workers-enabled", &paths.workers_enabled),
    ];

    let missing: Vec<&str> = dirs
        .iter()
        .filter(|(_, p)| !p.exists())
        .map(|(n, _)| *n)
        .collect();

    if missing.is_empty() {
        Check::ok(
            "directory structure",
            format!("all present under {}", paths.riku_root.display()),
        )
    } else {
        Check::fail(
            "directory structure",
            format!("missing: {}, run: riku init", missing.join(", ")),
        )
    }
}

/// Report whether the supervisor (`riku.service`) is active, checking the
/// user scope first then the system scope.
pub fn systemd_service() -> Check {
    if which::which("systemctl").is_err() {
        return Check::warn(
            "supervisor service",
            "systemctl unavailable, start manually: riku supervisor",
        );
    }

    let scopes: [(&[&str], &str); 2] = [
        (&["--user", "is-active", "riku"], "user"),
        (&["is-active", "riku"], "system"),
    ];

    for (args, scope) in scopes {
        if let Ok(o) = Command::new("systemctl").args(args).output() {
            if o.status.success() {
                return Check::ok(
                    "supervisor service",
                    format!("riku.service active ({scope})"),
                );
            }
        }
    }

    Check::warn(
        "supervisor service",
        "riku.service not active, start with: systemctl --user start riku",
    )
}

/// Count installed runtime plugins. Zero usually means `install-plugins`
/// has not been run, so apps cannot be built.
pub fn plugins(paths: &RikuPaths) -> Check {
    if !paths.plugin_root.exists() {
        return Check::warn(
            "runtime plugins",
            "plugins dir missing, run: riku install-plugins",
        );
    }

    let count = std::fs::read_dir(&paths.plugin_root)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .filter(|e| e.path().is_file())
                .count()
        })
        .unwrap_or(0);

    if count == 0 {
        Check::warn(
            "runtime plugins",
            "none installed, run: riku install-plugins",
        )
    } else {
        Check::ok("runtime plugins", format!("{count} installed"))
    }
}

/// Free space on the filesystem holding `~/.riku`. Builds and releases need
/// headroom; warn under 1 GiB, fail under 100 MiB.
pub fn disk(paths: &RikuPaths) -> Check {
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = MIB * 1024;

    let target = if paths.riku_root.exists() {
        paths.riku_root.as_path()
    } else {
        std::path::Path::new("/")
    };

    match statvfs(target) {
        Ok(st) => {
            // `statvfs` field widths differ by platform (u64 on linux-x86_64,
            // narrower elsewhere); cast keeps the arithmetic portable.
            #[allow(clippy::unnecessary_cast)]
            let free = st.blocks_available() as u64 * st.fragment_size() as u64;
            let detail = format!(
                "{:.1} GiB free on {}",
                free as f64 / GIB as f64,
                target.display()
            );
            if free < 100 * MIB {
                Check::fail("disk space", format!("only {detail}, deploys will fail"))
            } else if free < GIB {
                Check::warn("disk space", format!("low: {detail}"))
            } else {
                Check::ok("disk space", detail)
            }
        }
        Err(e) => Check::warn(
            "disk space",
            format!("could not stat {}: {e}", target.display()),
        ),
    }
}
