//! Individual diagnostic checks for `riku doctor`.
//!
//! Every function is **read-only**: it inspects the host (PATH, filesystem,
//! `systemctl`/`nginx` status) and reports, but never mutates Riku state.
//! Each returns one or more [`Check`] results carrying a remediation hint on
//! anything that is not [`Status::Ok`].

use std::process::Command;

use nix::sys::statvfs::statvfs;

use crate::config::RikuPaths;

use super::Check;

/// `command="FINGERPRINT=...` prefix that `setup_authorized_keys` writes for
/// every riku deploy key — used to distinguish riku keys from unrelated ones.
const RIKU_KEY_MARKER: &str = "FINGERPRINT=";

/// External tools Riku relies on: git is required, nginx and systemd are
/// strongly recommended but Riku degrades gracefully without them.
pub fn dependencies() -> Vec<Check> {
    let mut out = Vec::new();

    if which::which("git").is_ok() {
        out.push(Check::ok("git", "found on PATH"));
    } else {
        out.push(Check::fail(
            "git",
            "not found — git is required (apt install git)",
        ));
    }

    if which::which("nginx").is_ok() {
        out.push(Check::ok("nginx", "found on PATH"));
    } else {
        out.push(Check::warn(
            "nginx",
            "not found — web serving disabled (apt install nginx)",
        ));
    }

    if which::which("systemctl").is_ok() {
        out.push(Check::ok("systemd", "systemctl available"));
    } else {
        out.push(Check::warn(
            "systemd",
            "systemctl not found — run the supervisor manually: riku supervisor",
        ));
    }

    out
}

/// Verify the `~/.riku/` directory tree exists. A missing tree almost always
/// means `riku init` has not been run.
pub fn directories(paths: &RikuPaths) -> Check {
    let dirs: [(&str, &std::path::Path); 8] = [
        ("apps", &paths.app_root),
        ("data", &paths.data_root),
        ("envs", &paths.env_root),
        ("repos", &paths.git_root),
        ("logs", &paths.log_root),
        ("nginx", &paths.nginx_root),
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
            format!("missing: {} — run: riku init", missing.join(", ")),
        )
    }
}

/// Confirm the riku binary is reachable on PATH and installed where the
/// git post-receive hook expects it (`~/.local/bin/riku`).
pub fn binary() -> Vec<Check> {
    let mut out = Vec::new();

    match which::which("riku") {
        Ok(p) => out.push(Check::ok("riku on PATH", p.display().to_string())),
        Err(_) => out.push(Check::warn(
            "riku on PATH",
            "not on PATH — add ~/.local/bin to PATH",
        )),
    }

    if let Ok(home) = std::env::var("HOME") {
        let installed = std::path::Path::new(&home).join(".local/bin/riku");
        if installed.exists() {
            out.push(Check::ok(
                "installed binary",
                installed.display().to_string(),
            ));
        } else {
            out.push(Check::warn(
                "installed binary",
                format!("{} not found — run: riku init", installed.display()),
            ));
        }
    }

    out
}

/// Report whether the supervisor (`riku.service`) is active, checking the
/// user scope first then the system scope.
pub fn systemd_service() -> Check {
    if which::which("systemctl").is_err() {
        return Check::warn(
            "supervisor service",
            "systemctl unavailable — start manually: riku supervisor",
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
        "riku.service not active — start with: systemctl --user start riku",
    )
}

/// Validate the generated nginx configuration via `nginx -t`. Skipped (empty
/// result) when nginx is absent — `dependencies` already reports that.
pub fn nginx() -> Vec<Check> {
    if which::which("nginx").is_err() {
        return Vec::new();
    }

    match Command::new("nginx").arg("-t").output() {
        Ok(o) if o.status.success() => vec![Check::ok("nginx config", "nginx -t passed")],
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            // Without root, `nginx -t` cannot read its config and fails on a
            // permission error rather than an actual config problem.
            if stderr.to_lowercase().contains("permission denied") {
                vec![Check::warn(
                    "nginx config",
                    "cannot validate without root — re-run: sudo riku doctor",
                )]
            } else {
                let last = stderr
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("invalid configuration")
                    .trim();
                vec![Check::fail(
                    "nginx config",
                    format!("nginx -t failed: {last}"),
                )]
            }
        }
        Err(e) => vec![Check::warn(
            "nginx config",
            format!("could not run nginx -t: {e}"),
        )],
    }
}

/// Count installed runtime plugins. Zero usually means `install-plugins`
/// has not been run, so apps cannot be built.
pub fn plugins(paths: &RikuPaths) -> Check {
    if !paths.plugin_root.exists() {
        return Check::warn(
            "runtime plugins",
            "plugins dir missing — run: riku install-plugins",
        );
    }

    let count = std::fs::read_dir(&paths.plugin_root)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .count()
        })
        .unwrap_or(0);

    if count == 0 {
        Check::warn(
            "runtime plugins",
            "none installed — run: riku install-plugins",
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
                Check::fail("disk space", format!("only {detail} — deploys will fail"))
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

/// Check that at least one riku deploy key is authorized for `git push`.
pub fn ssh_access() -> Check {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Check::warn("ssh access", "HOME not set — cannot locate authorized_keys"),
    };

    let authorized_keys = std::path::Path::new(&home).join(".ssh/authorized_keys");
    if !authorized_keys.exists() {
        return Check::warn(
            "ssh access",
            "no authorized_keys — git push deploys need a key (riku init)",
        );
    }

    match std::fs::read_to_string(&authorized_keys) {
        Ok(content) => {
            let keys = content
                .lines()
                .filter(|l| l.contains(RIKU_KEY_MARKER))
                .count();
            if keys > 0 {
                Check::ok(
                    "ssh access",
                    format!("{keys} riku deploy key(s) authorized"),
                )
            } else {
                Check::warn(
                    "ssh access",
                    "authorized_keys present but no riku deploy keys — add one via riku init",
                )
            }
        }
        Err(e) => Check::warn("ssh access", format!("could not read authorized_keys: {e}")),
    }
}
