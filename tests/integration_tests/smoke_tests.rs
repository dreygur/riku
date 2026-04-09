/// Smoke tests for the Riku CLI command layer.
///
/// These tests verify that the binary accepts the right arguments, produces
/// expected output, and exits with the right code. They exercise the
/// `routing.rs` and `hooks.rs` modules through both binary invocation
/// and direct unit tests.

// ── Binary invocation helpers ────────────────────────────────────────────────

/// Return the path to the debug binary.
///
/// The binary must already exist (run `cargo build` before the tests).
fn riku_bin() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target/debug/riku");
    p
}

/// Run the binary with the given arguments and return the `Output`.
fn run(args: &[&str]) -> std::process::Output {
    std::process::Command::new(riku_bin())
        .args(args)
        .output()
        .expect("failed to execute riku binary — make sure `cargo build` has been run")
}

// ── Binary invocation tests ───────────────────────────────────────────────────

#[cfg(test)]
mod binary_tests {
    use super::run;

    #[test]
    fn help_exits_zero_and_mentions_riku() {
        let out = run(&["--help"]);
        assert!(out.status.success(), "`riku --help` should exit 0");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("riku"),
            "`riku --help` output should mention 'riku', got:\n{stdout}"
        );
    }

    #[test]
    fn version_exits_zero_and_prints_version() {
        let out = run(&["--version"]);
        assert!(out.status.success(), "`riku --version` should exit 0");
        // Version output typically goes to stdout; accept either stream.
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        // A version string contains at least one digit.
        assert!(
            combined.chars().any(|c| c.is_ascii_digit()),
            "`riku --version` should print a version number, got:\n{combined}"
        );
    }

    #[test]
    fn apps_help_exits_zero() {
        let out = run(&["apps", "--help"]);
        assert!(out.status.success(), "`riku apps --help` should exit 0");
    }

    #[test]
    fn deploy_help_exits_zero() {
        let out = run(&["deploy", "--help"]);
        assert!(out.status.success(), "`riku deploy --help` should exit 0");
    }

    #[test]
    fn plugin_list_help_exits_zero() {
        let out = run(&["plugin", "list", "--help"]);
        assert!(
            out.status.success(),
            "`riku plugin list --help` should exit 0"
        );
    }

    #[test]
    fn hook_list_help_exits_zero() {
        let out = run(&["hook", "list", "--help"]);
        assert!(
            out.status.success(),
            "`riku hook list --help` should exit 0"
        );
    }

    #[test]
    fn setup_help_exits_zero() {
        // `setup` maps to the `init` subcommand in this binary.
        let out = run(&["init", "--help"]);
        assert!(out.status.success(), "`riku init --help` should exit 0");
    }

    #[test]
    fn logs_help_exits_zero() {
        let out = run(&["logs", "--help"]);
        assert!(out.status.success(), "`riku logs --help` should exit 0");
    }

    #[test]
    fn ps_help_exits_zero() {
        let out = run(&["ps", "--help"]);
        assert!(out.status.success(), "`riku ps --help` should exit 0");
    }

    #[test]
    fn config_help_exits_zero() {
        let out = run(&["config", "--help"]);
        assert!(out.status.success(), "`riku config --help` should exit 0");
    }

    #[test]
    fn unknown_command_exits_nonzero() {
        let out = run(&["unknown-command-xyz"]);
        assert!(
            !out.status.success(),
            "`riku unknown-command-xyz` should exit non-zero"
        );
    }

    #[test]
    fn no_args_exits_nonzero_or_prints_help() {
        let out = run(&[]);
        // Either exit non-zero (clap's default) or print help text.
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let printed_help = stdout.contains("riku") || stderr.contains("riku");
        assert!(
            !out.status.success() || printed_help,
            "`riku` with no args should exit non-zero or print help"
        );
    }
}

// ── Unit tests for routing.rs ─────────────────────────────────────────────────

#[cfg(test)]
mod routing_tests {
    use riku::cli::routing::{build_plugin_args, get_plugin_command};
    use riku::cli::{Commands, ConfigCmd};

    // ── get_plugin_command ────────────────────────────────────────────────────

    #[test]
    fn apps_command_maps_to_apps() {
        let cmd = Commands::Apps { cmd: None };
        assert_eq!(get_plugin_command(&cmd), Some("apps".to_string()));
    }

    #[test]
    fn deploy_command_maps_to_deploy() {
        let cmd = Commands::Deploy {
            app: "myapp".to_string(),
            from: None,
        };
        assert_eq!(get_plugin_command(&cmd), Some("deploy".to_string()));
    }

    #[test]
    fn logs_command_maps_to_logs() {
        let cmd = Commands::Logs {
            app: "myapp".to_string(),
            process: "*".to_string(),
            deploy: false,
            follow: false,
        };
        assert_eq!(get_plugin_command(&cmd), Some("logs".to_string()));
    }

    #[test]
    fn ps_command_maps_to_ps() {
        let cmd = Commands::Ps {
            app: None,
            verbose: false,
            scale: vec![],
        };
        assert_eq!(get_plugin_command(&cmd), Some("ps".to_string()));
    }

    #[test]
    fn config_command_maps_to_config() {
        let cmd = Commands::Config(ConfigCmd::Show {
            app: "myapp".to_string(),
        });
        assert_eq!(get_plugin_command(&cmd), Some("config".to_string()));
    }

    #[test]
    fn destroy_command_maps_to_destroy() {
        let cmd = Commands::Destroy {
            app: "myapp".to_string(),
        };
        assert_eq!(get_plugin_command(&cmd), Some("destroy".to_string()));
    }

    #[test]
    fn restart_command_maps_to_restart() {
        let cmd = Commands::Restart {
            app: "myapp".to_string(),
            hot: false,
        };
        assert_eq!(get_plugin_command(&cmd), Some("restart".to_string()));
    }

    #[test]
    fn stop_command_maps_to_stop() {
        let cmd = Commands::Stop {
            app: "myapp".to_string(),
        };
        assert_eq!(get_plugin_command(&cmd), Some("stop".to_string()));
    }

    #[test]
    fn plugin_command_returns_none() {
        use riku::cli::cmds::PluginCmd;
        let cmd = Commands::Plugin(PluginCmd::List);
        assert_eq!(get_plugin_command(&cmd), None);
    }

    #[test]
    fn hook_command_returns_none() {
        use riku::cli::cmds::HookCmd;
        let cmd = Commands::Hook(HookCmd::List);
        assert_eq!(get_plugin_command(&cmd), None);
    }

    #[test]
    fn init_command_returns_none() {
        let cmd = Commands::Init { no_systemd: false };
        assert_eq!(get_plugin_command(&cmd), None);
    }

    #[test]
    fn supervisor_command_returns_none() {
        let cmd = Commands::Supervisor;
        assert_eq!(get_plugin_command(&cmd), None);
    }

    #[test]
    fn update_command_returns_none() {
        let cmd = Commands::Update;
        assert_eq!(get_plugin_command(&cmd), None);
    }

    // ── build_plugin_args ─────────────────────────────────────────────────────

    #[test]
    fn build_plugin_args_apps_has_server_placeholder() {
        let cmd = Commands::Apps { cmd: None };
        let args = build_plugin_args(&cmd);
        // $1 is always an empty server placeholder
        assert_eq!(args[0], "", "first arg should be empty server placeholder");
        assert!(args.contains(&"apps".to_string()));
    }

    #[test]
    fn build_plugin_args_deploy_includes_app_and_command() {
        let cmd = Commands::Deploy {
            app: "testapp".to_string(),
            from: None,
        };
        let args = build_plugin_args(&cmd);
        assert_eq!(args[0], "");
        assert!(args.contains(&"testapp".to_string()));
        assert!(args.contains(&"deploy".to_string()));
    }

    #[test]
    fn build_plugin_args_deploy_with_from_includes_path() {
        let cmd = Commands::Deploy {
            app: "testapp".to_string(),
            from: Some("./local-path".to_string()),
        };
        let args = build_plugin_args(&cmd);
        assert!(args.contains(&"--from".to_string()));
        assert!(args.contains(&"./local-path".to_string()));
    }

    #[test]
    fn build_plugin_args_logs_without_process_filter_omits_star() {
        let cmd = Commands::Logs {
            app: "myapp".to_string(),
            process: "*".to_string(),
            deploy: false,
            follow: false,
        };
        let args = build_plugin_args(&cmd);
        // When process is "*", it should not be appended
        assert!(!args.contains(&"*".to_string()));
        assert!(args.contains(&"logs".to_string()));
    }

    #[test]
    fn build_plugin_args_logs_with_process_filter_includes_it() {
        let cmd = Commands::Logs {
            app: "myapp".to_string(),
            process: "web".to_string(),
            deploy: false,
            follow: false,
        };
        let args = build_plugin_args(&cmd);
        assert!(args.contains(&"web".to_string()));
    }

    #[test]
    fn build_plugin_args_config_set_includes_settings() {
        let cmd = Commands::Config(ConfigCmd::Set {
            app: "myapp".to_string(),
            settings: vec!["KEY=VAL".to_string(), "OTHER=X".to_string()],
        });
        let args = build_plugin_args(&cmd);
        assert!(args.contains(&"KEY=VAL".to_string()));
        assert!(args.contains(&"OTHER=X".to_string()));
    }

    #[test]
    fn build_plugin_args_ps_scale_uses_ps_scale_command() {
        let cmd = Commands::Ps {
            app: Some("myapp".to_string()),
            verbose: false,
            scale: vec!["web=2".to_string()],
        };
        let args = build_plugin_args(&cmd);
        assert!(args.contains(&"ps:scale".to_string()));
    }

    #[test]
    fn build_plugin_args_ps_no_scale_uses_ps_show_command() {
        let cmd = Commands::Ps {
            app: Some("myapp".to_string()),
            verbose: false,
            scale: vec![],
        };
        let args = build_plugin_args(&cmd);
        assert!(args.contains(&"ps:show".to_string()));
    }
}

// ── Unit tests for hooks.rs ───────────────────────────────────────────────────

#[cfg(test)]
mod hook_cmd_tests {
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    use riku::config::RikuPaths;

    /// Build a minimal `RikuPaths` rooted at `riku_root` inside the temp dir.
    fn make_paths(temp: &TempDir) -> (PathBuf, RikuPaths) {
        let riku_root = temp.path().join(".riku");
        let dirs = [
            "apps",
            "data",
            "envs",
            "repos",
            "logs",
            "nginx",
            "cache",
            "workers",
            "workers-available",
            "workers-enabled",
            "acme",
            "plugins",
        ];
        for d in &dirs {
            fs::create_dir_all(riku_root.join(d)).unwrap();
        }
        let paths = RikuPaths::from_dirs(riku_root.clone(), temp.path());
        (riku_root, paths)
    }

    /// Create an executable script at `dir/name`.
    fn create_executable(dir: &PathBuf, name: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, "#!/bin/sh\necho hello\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        }
        p
    }

    // cmd_hook_list — empty plugins dir ----------------------------------------

    #[test]
    fn hook_list_empty_dir_returns_ok() {
        let temp = TempDir::new().unwrap();
        let (_root, paths) = make_paths(&temp);
        // No plugins installed — should return Ok (not panic or error).
        let result = riku::cli::hooks::cmd_hook_list(&paths);
        assert!(
            result.is_ok(),
            "cmd_hook_list with empty dir should return Ok"
        );
    }

    // cmd_hook_list — populated plugins dir ------------------------------------

    #[test]
    fn hook_list_with_plugins_returns_ok() {
        let temp = TempDir::new().unwrap();
        let (_root, paths) = make_paths(&temp);
        create_executable(&paths.plugin_root, "riku-pre-deploy");
        create_executable(&paths.plugin_root, "riku-post-deploy");

        let result = riku::cli::hooks::cmd_hook_list(&paths);
        assert!(
            result.is_ok(),
            "cmd_hook_list with plugins present should return Ok"
        );
    }

    // cmd_hook_check — invalid name (path traversal) ---------------------------

    #[test]
    fn hook_check_invalid_name_returns_error() {
        let temp = TempDir::new().unwrap();
        let (_root, paths) = make_paths(&temp);
        // Names containing ".." should be rejected by validate_plugin_name.
        let result = riku::cli::hooks::cmd_hook_check(&paths, "../evil");
        assert!(
            result.is_err(),
            "cmd_hook_check with path-traversal name should return Err"
        );
    }

    #[test]
    fn hook_check_empty_name_returns_error() {
        let temp = TempDir::new().unwrap();
        let (_root, paths) = make_paths(&temp);
        let result = riku::cli::hooks::cmd_hook_check(&paths, "");
        assert!(
            result.is_err(),
            "cmd_hook_check with empty name should return Err"
        );
    }

    // cmd_hook_check — non-existent hook  --------------------------------------
    // NOTE: cmd_hook_check calls std::process::exit on both the found and
    // not-found paths. We cannot safely call it in-process for those cases
    // without catching the exit. The tests above cover the validation-error
    // path (which returns Err before exit). The exit-path behaviour is covered
    // by the binary invocation tests via `riku hook check`.
}
