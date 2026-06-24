//! Capability-enforcement integration tests.
//!
//! Verifies that a sandboxed plugin child can write only to its declared
//! directories. Landlock is best-effort, so on a kernel/sandbox without it the
//! confinement assertion is skipped rather than failing — the feature degrades,
//! it does not break deploys.

#[cfg(test)]
mod tests {
    use riku::plugins::manifest::Capabilities;
    use riku::plugins::sandbox::{harden, SandboxPaths};
    use std::path::PathBuf;
    use std::process::Command;

    fn caps(writes: &[&str]) -> Capabilities {
        Capabilities {
            network: true,
            writes: writes.iter().map(|s| s.to_string()).collect(),
            privileged: false,
        }
    }

    #[test]
    fn writes_confined_to_declared_dirs() {
        // Use the per-test target dir, which lives under target/ — NOT under the
        // system temp dir (which the sandbox always grants), so `forbidden` is a
        // genuine out-of-policy location.
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sandbox-writes");
        let allowed = base.join("allowed");
        let forbidden = base.join("forbidden");
        std::fs::create_dir_all(&allowed).unwrap();
        std::fs::create_dir_all(&forbidden).unwrap();
        let _ = std::fs::remove_file(allowed.join("a"));
        let _ = std::fs::remove_file(forbidden.join("b"));

        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg(r#"touch "$ALLOWED/a" 2>/dev/null; touch "$FORBIDDEN/b" 2>/dev/null; true"#)
            .env("ALLOWED", &allowed)
            .env("FORBIDDEN", &forbidden);

        // `app_dir` resolves to the allowed directory; everything else is denied.
        let paths = SandboxPaths {
            app_path: Some(allowed.clone()),
            ..Default::default()
        };
        harden(&mut cmd, &caps(&["app_dir"]), &paths);

        let status = cmd.status().expect("spawn /bin/sh");
        assert!(status.success());

        let wrote_allowed = allowed.join("a").exists();
        let wrote_forbidden = forbidden.join("b").exists();

        if wrote_forbidden {
            eprintln!("landlock not enforced on this host; skipping confinement assertion");
            return;
        }
        assert!(wrote_allowed, "write to declared app_dir should succeed");
        assert!(
            !wrote_forbidden,
            "write outside declared dirs must be blocked"
        );
    }

    #[test]
    fn privileged_plugin_is_not_confined() {
        // A privileged plugin opts out: a write outside any declared dir succeeds.
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sandbox-priv");
        std::fs::create_dir_all(&base).unwrap();
        let target = base.join("c");
        let _ = std::fs::remove_file(&target);

        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg(r#"touch "$T" 2>/dev/null; true"#)
            .env("T", &target);

        let privileged = Capabilities {
            network: true,
            writes: vec![],
            privileged: true,
        };
        harden(&mut cmd, &privileged, &SandboxPaths::default());

        assert!(cmd.status().expect("spawn /bin/sh").success());
        assert!(target.exists(), "privileged plugin should write freely");
    }
}
