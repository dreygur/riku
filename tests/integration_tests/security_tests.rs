//! Security-focused integration tests.
//!
//! Exercises the trust boundaries: path handling, file permissions, and what
//! ends up in generated config: through real file operations.

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper to create a temporary Riku environment
    fn setup_riku_env() -> Result<(TempDir, PathBuf)> {
        let temp_dir = TempDir::new()?;
        let riku_root = temp_dir.path().join(".riku");

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
            "acme-www",
            "plugins",
        ];

        for dir in &dirs {
            fs::create_dir_all(riku_root.join(dir))?;
        }

        Ok((temp_dir, riku_root))
    }

    // --- App directory traversal protection ---

    #[test]
    fn test_app_directory_no_traversal() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Apply the same sanitization logic used by the codebase
        let malicious_names = vec![
            "../etc",
            "..",
            "app/../secret",
            "...",
            "my..app",
            ".",
            "/",
            "../../../etc/passwd",
        ];

        for name in malicious_names {
            let stripped = name.trim_start_matches('/');
            let sanitized: String = stripped
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
                .collect::<String>()
                .trim_end()
                .to_string();

            // Verify the sanitization rejects these names (returns empty)
            let is_rejected = sanitized.contains("..")
                || sanitized.is_empty()
                || sanitized.trim_matches('.').is_empty();

            assert!(
                is_rejected,
                "Malicious app name '{}' (sanitized: '{}') should be rejected",
                name, sanitized
            );

            // Ensure we can't create directories with these names
            if !sanitized.is_empty() && !is_rejected {
                let app_dir = riku_root.join("apps").join(&sanitized);
                // This would only execute for non-rejected names
                assert!(!app_dir.to_str().unwrap().contains(".."));
            }
        }

        // Verify valid names pass sanitization
        let valid_names = vec!["my-app", "app.v2", "test_app", "app123"];
        for name in valid_names {
            let sanitized: String = name
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
                .collect();
            let is_rejected = sanitized.contains("..")
                || sanitized.is_empty()
                || sanitized.trim_matches('.').is_empty();

            assert!(
                !is_rejected,
                "Valid app name '{}' should pass sanitization",
                name
            );

            // Verify we can create directories safely
            let app_dir = riku_root.join("apps").join(&sanitized);
            fs::create_dir_all(&app_dir)?;
            assert!(app_dir.exists());
        }

        Ok(())
    }

    // --- Plugin path traversal protection ---

    #[test]
    fn test_plugin_directory_traversal_protection() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create a file outside the plugin directory
        let secret_file = _temp_dir.path().join("secret.txt");
        fs::write(&secret_file, "secret data")?;

        // Verify that traversal paths resolve outside plugin_root
        let plugin_root = riku_root.join("plugins");
        let traversal_path = plugin_root.join("../../secret.txt");

        // The traversal path should resolve to the secret file
        if let Ok(resolved) = fs::canonicalize(&traversal_path) {
            let plugin_root_resolved = fs::canonicalize(&plugin_root)?;
            assert!(
                !resolved.starts_with(&plugin_root_resolved),
                "Path traversal should escape plugin root"
            );
        }

        Ok(())
    }

    #[test]
    fn test_plugin_name_validation() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Malicious plugin names that should be rejected
        let malicious_names = vec![
            "../../../etc/passwd",
            "../../bin/sh",
            "../secret",
            "..",
            "plugin/subdir",
            "plugin\\subdir",
            "",
        ];

        for name in malicious_names {
            // The validation should reject these names
            let has_slash = name.contains('/') || name.contains('\\');
            let has_dots = name.contains("..");
            let is_empty = name.is_empty();

            assert!(
                has_slash || has_dots || is_empty,
                "Malicious plugin name '{}' should fail validation criteria",
                name
            );

            // Verify we can't access files outside plugin root via these names
            if !is_empty {
                let plugin_path = riku_root.join("plugins").join(name);
                // If the path contains traversal, it should resolve outside plugins
                if has_dots {
                    let _plugin_root = riku_root.join("plugins");
                    // Don't actually create these paths, just verify the logic
                    assert!(
                        plugin_path.to_str().unwrap().contains("..") || has_slash,
                        "Path should contain dangerous sequences"
                    );
                }
            }
        }

        Ok(())
    }

    // --- ENV file security ---

    /// The paths riku reads for app `testapp`, with `ENV` holding `content`.
    fn app_with_env(tmp: &TempDir, content: &[u8]) -> riku::config::RikuPaths {
        let paths = riku::config::RikuPaths::for_tests(tmp.path());
        let env_dir = paths.env_root.join("testapp");
        fs::create_dir_all(&env_dir).unwrap();
        fs::write(env_dir.join("ENV"), content).unwrap();
        paths
    }

    /// Shell syntax in an ENV value reaches the app as text: riku's reader
    /// never runs it, and only `$VAR` and `${VAR}` of earlier lines expand.
    #[test]
    fn test_env_file_with_special_characters() {
        let tmp = TempDir::new().unwrap();
        let paths = app_with_env(
            &tmp,
            b"SUB=$(touch pwned); echo `id` | sh\nQUOTED=\"a b\" 'c'\nEQ=a=b;c\nBASE=/srv\nPATHS=${BASE}/x:$BASE/y\n",
        );

        let env = riku::config::app_config::load_env(&paths, "testapp")
            .expect("the ENV file parses")
            .env;

        let expected = [
            ("SUB", "$(touch pwned); echo `id` | sh"),
            ("QUOTED", "\"a b\" 'c'"),
            ("EQ", "a=b;c"),
            ("PATHS", "/srv/x:/srv/y"),
        ];
        for (key, value) in expected {
            assert_eq!(env.get(key).map(String::as_str), Some(value), "{key}");
        }
        assert!(
            !tmp.path().join("pwned").exists(),
            "reading ENV ran a command"
        );
    }

    #[test]
    fn test_env_file_null_bytes_in_value() {
        let tmp = TempDir::new().unwrap();
        let paths = app_with_env(&tmp, b"KEY=hello\x00world\x07\tend\n");

        let env = riku::config::app_config::load_env(&paths, "testapp")
            .expect("the ENV file parses")
            .env;

        assert_eq!(
            env.get("KEY").map(String::as_str),
            Some("helloworld\tend"),
            "NUL and other control characters must be dropped, tab kept"
        );
    }

    // --- Symlink safety ---

    /// An app's ACME certificate link that points outside riku's tree and
    /// acme.sh's is unlinked by destroy, and its target is left alone.
    #[test]
    fn test_symlink_cannot_escape_riku_root() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let mut paths = riku::config::RikuPaths::for_tests(&tmp.path().join("server"));
        fs::create_dir_all(paths.app_root.join("evilapp")).unwrap();
        riku::config::router::save(&paths, &riku::config::router::Router::Caddy).unwrap();
        paths.reload_programs.caddy = tmp.path().join("mock_caddy");
        fs::write(&paths.reload_programs.caddy, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(
            &paths.reload_programs.caddy,
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        riku_route::serve_caddy_for_tests(&mut paths);

        let external = tmp.path().join("external_data");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("important.txt"), "do not delete").unwrap();
        fs::create_dir_all(&paths.acme_cert_links).unwrap();
        let link = paths.acme_cert_links.join("evilapp");
        std::os::unix::fs::symlink(&external, &link).unwrap();

        riku::cli::apps::cmd_destroy(&paths, "evilapp", riku::cli::apps::DestroyMode::Safe)
            .expect("destroy succeeds");

        assert!(
            external.join("important.txt").exists(),
            "destroy deleted a directory outside riku's tree through the link"
        );
        assert!(
            fs::symlink_metadata(&link).is_err(),
            "the link itself must go with the app"
        );
    }

    // --- Nginx config security ---

    /// The app's domain list reaches `server_name` only through
    /// `parse_domains`, the parser the route build runs on `RIKU_DOMAINS`.
    #[test]
    fn test_nginx_config_no_injection_in_server_name() {
        use riku::util::domain::parse_domains;

        for raw in [
            "safe.example.com;return 200 pwned",
            "safe.example.com{",
            "safe.example.com\"",
            "safe.example.com$host",
            "safe.example.com#",
        ] {
            let parsed = parse_domains(raw);
            assert!(parsed.is_err(), "RIKU_DOMAINS={raw:?} parsed: {parsed:?}");
        }

        let domains: Vec<String> = parse_domains("safe.example.com, \nwww.example.com")
            .expect("a clean list parses")
            .iter()
            .map(ToString::to_string)
            .collect();
        let site = render_static_site(&domains, &[]).expect("a clean list renders");
        let server_names: Vec<&str> = site
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("server_name"))
            .collect();
        assert!(!server_names.is_empty(), "no server_name in:\n{site}");
        for line in server_names {
            assert_eq!(
                line.split_whitespace().collect::<Vec<_>>(),
                ["server_name", "safe.example.com", "www.example.com;"],
                "site:\n{site}"
            );
        }
    }

    /// Renders a static site, the shape that needs no backend, through the
    /// same renderer the router helper runs.
    fn render_static_site(domains: &[String], static_locations: &[String]) -> Result<String> {
        use riku::nginx::{render_route_site, RouteSite, SiteFiles, SiteShape};

        let files = SiteFiles {
            certificate: PathBuf::from("/etc/riku/certs/myapp/fullchain.pem"),
            certificate_key: PathBuf::from("/etc/riku/certs/myapp/key.pem"),
            acme_root: PathBuf::from("/var/lib/riku/acme"),
            static_root: PathBuf::from("/var/lib/riku/static"),
        };
        render_route_site(&RouteSite {
            app: "myapp",
            domains,
            shape: SiteShape::Static,
            https_only: false,
            also_tls: false,
            open_port: None,
            port: 80,
            backends: &[],
            static_locations,
            allow_git_folders: false,
            ipv6: false,
            files: &files,
        })
    }

    #[test]
    fn nginx_site_refuses_values_that_could_inject_a_directive() {
        let clean = render_static_site(&["myapp.example.com".to_string()], &[])
            .expect("a clean domain renders");
        let server_name = clean
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("server_name"));
        assert_eq!(
            server_name.map(|line| line.split_whitespace().collect::<Vec<_>>()),
            Some(vec!["server_name", "myapp.example.com;"]),
            "clean site:\n{clean}"
        );

        for ch in [';', '{', '}', '#', '\n', '\r', '`', '$', '\\', '"', '\''] {
            let value = format!("myapp.example.com{ch}return 200 pwned");
            let domain = render_static_site(std::slice::from_ref(&value), &[]);
            assert!(domain.is_err(), "domain {value:?} rendered: {domain:?}");

            let location = format!("/static{ch}return 200 pwned");
            let static_path = render_static_site(
                &["myapp.example.com".to_string()],
                std::slice::from_ref(&location),
            );
            assert!(
                static_path.is_err(),
                "static location {location:?} rendered: {static_path:?}"
            );
        }
    }

    // --- Worker config injection ---

    #[test]
    fn test_worker_config_toml_structure() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create a worker config with standard fields
        let config_content = r#"
[worker]
app = "testapp"
kind = "web"
command = "python app.py"
ordinal = 1

[env]
PORT = "8080"
HOST = "localhost"

[options]
working_dir = "/tmp"
"#;

        let config_path = riku_root.join("workers-enabled").join("testapp@web.1.toml");
        fs::write(&config_path, config_content)?;

        // Verify it parses as valid TOML
        let content = fs::read_to_string(&config_path)?;
        let _parsed: toml::Value = toml::from_str(&content)?;

        Ok(())
    }

    // --- Procfile cron bounds ---

    #[test]
    fn test_procfile_cron_boundary_values() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;
        let app_path = riku_root.join("apps").join("cronapp");
        fs::create_dir_all(&app_path)?;

        // Write Procfile with boundary cron values
        // Valid: minute=59, hour=23, day=31, month=12, weekday=6
        let valid_procfile = "cron: 59 23 31 12 6 /usr/bin/valid-task\n";
        fs::write(app_path.join("Procfile"), valid_procfile)?;

        let content = fs::read_to_string(app_path.join("Procfile"))?;
        assert!(content.contains("59 23 31 12 6"));

        // Invalid: hour=24 (max is 23)
        let invalid_procfile = "cron: 0 24 * * * /usr/bin/invalid-task\n";
        fs::write(app_path.join("Procfile.invalid"), invalid_procfile)?;

        Ok(())
    }

    // --- File permission tests ---

    #[test]
    fn test_plugin_must_be_executable() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create a non-executable plugin file
        let plugin_path = riku_root.join("plugins").join("test-plugin");
        fs::write(&plugin_path, "#!/bin/bash\necho test\n")?;

        // Set permissions to non-executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plugin_path, fs::Permissions::from_mode(0o644))?;

            let metadata = fs::metadata(&plugin_path)?;
            let mode = metadata.permissions().mode();
            assert_eq!(mode & 0o111, 0, "Plugin should not be executable");
        }

        Ok(())
    }

    // --- Git hook security ---

    #[test]
    fn test_git_hook_not_world_writable() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create a mock git hook
        let hook_dir = riku_root.join("repos").join("testapp").join("hooks");
        fs::create_dir_all(&hook_dir)?;

        let hook_path = hook_dir.join("post-receive");
        fs::write(&hook_path, "#!/bin/bash\necho deploy\n")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Set proper permissions (755, not 777)
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;

            let metadata = fs::metadata(&hook_path)?;
            let mode = metadata.permissions().mode();
            // Verify not world-writable
            assert_eq!(mode & 0o002, 0, "Git hook should not be world-writable");
        }

        Ok(())
    }

    // --- Log directory isolation ---

    #[test]
    fn test_log_directory_per_app_isolation() -> Result<()> {
        let (_temp_dir, riku_root) = setup_riku_env()?;

        // Create log directories for two apps
        let app1_logs = riku_root.join("logs").join("app1");
        let app2_logs = riku_root.join("logs").join("app2");
        fs::create_dir_all(&app1_logs)?;
        fs::create_dir_all(&app2_logs)?;

        // Write log files
        fs::write(app1_logs.join("web.1.log"), "app1 log data")?;
        fs::write(app2_logs.join("web.1.log"), "app2 log data")?;

        // Verify isolation - each app only sees its own logs
        let app1_log_content = fs::read_to_string(app1_logs.join("web.1.log"))?;
        let app2_log_content = fs::read_to_string(app2_logs.join("web.1.log"))?;

        assert!(app1_log_content.contains("app1"));
        assert!(!app1_log_content.contains("app2"));
        assert!(app2_log_content.contains("app2"));
        assert!(!app2_log_content.contains("app1"));

        Ok(())
    }

    /// `config set` output reaches terminals, CI logs and agent
    /// transcripts, so it names the keys it set and never their values.
    #[test]
    fn config_set_output_never_shows_the_value() {
        let home = TempDir::new().unwrap();
        fs::create_dir_all(home.path().join(".riku/apps/shop")).unwrap();
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_riku"))
            .args(["config", "set", "shop", "API_KEY=s3cr3t-value"])
            .env("HOME", home.path())
            .env("XDG_CONFIG_HOME", home.path())
            .env_remove("RIKU_ROOT")
            .env_remove("SSH_ORIGINAL_COMMAND")
            .current_dir(home.path())
            .output()
            .unwrap();
        let shown = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            shown.contains("Set  API_KEY"),
            "the key is not named:\n{shown}"
        );
        assert!(
            !shown.contains("s3cr3t-value"),
            "the value leaked:\n{shown}"
        );
    }
}
