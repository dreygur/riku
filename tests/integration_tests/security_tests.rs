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

    // --- App directory traversal protection ---

    /// `riku apps create` refuses a name that would leave `apps/` or `repos/`
    /// before it creates anything, so no directory appears anywhere.
    #[test]
    fn apps_create_refuses_a_traversing_name_and_creates_nothing() {
        let tmp = TempDir::new().unwrap();
        let paths = riku::config::RikuPaths::for_tests(tmp.path());

        for name in [
            "../etc",
            "..",
            "app/../secret",
            "...",
            "my..app",
            ".",
            "/",
            "../../../etc/passwd",
        ] {
            assert!(
                riku::cli::apps::cmd_apps_create(&paths, name).is_err(),
                "apps create accepted the app name {name:?}"
            );
        }

        let created: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert!(created.is_empty(), "refused names created {created:?}");
    }

    // --- Plugin path traversal protection ---

    /// An executable outside the plugin directory is not a plugin, even when
    /// a `..` name reaches it.
    #[test]
    fn a_plugin_name_cannot_reach_an_executable_outside_the_plugin_directory() {
        let tmp = TempDir::new().unwrap();
        let paths = riku::config::RikuPaths::for_tests(tmp.path());
        fs::create_dir_all(&paths.plugin_root).unwrap();
        let outside = paths.plugin_root.join("../evil");
        fs::write(&outside, "#!/bin/sh\n").unwrap();
        fs::set_permissions(
            &outside,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();

        assert!(
            !riku::plugins::plugin_exists("../evil", &paths),
            "plugin_exists followed ../evil out of {}",
            paths.plugin_root.display()
        );
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

    /// A Procfile command and ENV values reach the worker config as TOML
    /// strings, so quotes and newlines in them cannot add a section or key,
    /// such as a `uid` the supervisor would run the worker as.
    #[test]
    fn a_hostile_command_or_env_value_cannot_add_keys_to_a_worker_config() {
        use riku::supervisor::config::{create_worker_config, WorkerConfig};
        use std::collections::HashMap;

        let command = "python app.py\"\n[options]\nuid = \"0\"\n#";
        let value = "x\"\nRIKU_SANDBOX = \"off\"\n[options]\ngid = \"0";
        let env = HashMap::from([("GREETING".to_string(), value.to_string())]);
        let config = create_worker_config("shop", "web", command, 1, env, "/srv/shop", "/log");

        // The deploy writes worker configs with `toml::to_string`.
        let text = toml::to_string(&config).expect("the config serializes");
        let read: WorkerConfig = toml::from_str(&text).expect("the config reads back");

        assert_eq!(read.worker.command, command, "config:\n{text}");
        assert_eq!(
            read.env.get("GREETING").map(String::as_str),
            Some(value),
            "config:\n{text}"
        );
        assert_eq!(
            (read.options.uid, read.options.gid),
            (None, None),
            "config:\n{text}"
        );
        assert!(
            !read.env.contains_key("RIKU_SANDBOX"),
            "an ENV value added a key:\n{text}"
        );
    }

    /// Runtime discovery has its own test (riku-plugins
    /// `discover_skips_non_executable`); this covers the lookup `riku plugin
    /// list` and `exists` use.
    #[test]
    fn a_non_executable_file_in_the_plugin_directory_is_not_a_plugin() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let paths = riku::config::RikuPaths::for_tests(tmp.path());
        fs::create_dir_all(&paths.plugin_root).unwrap();
        let plugin = paths.plugin_root.join("notes");
        fs::write(&plugin, "#!/bin/sh\necho ran\n").unwrap();
        fs::set_permissions(&plugin, fs::Permissions::from_mode(0o644)).unwrap();

        assert!(!riku::plugins::plugin_exists("notes", &paths));
        assert_eq!(
            riku::plugins::list_plugins(&paths).unwrap(),
            Vec::<String>::new()
        );
    }

    /// The post-receive hook runs on every push as the deploy user, so no
    /// other account may change it. riku sets the mode itself rather than
    /// leaving it to the umask.
    #[test]
    fn the_post_receive_hook_riku_writes_is_writable_only_by_its_owner() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let paths = riku::config::RikuPaths::for_tests(tmp.path());
        riku::cli::apps::cmd_apps_create(&paths, "shop").expect("apps create succeeds");

        let hook = paths.git_root.join("shop.git/hooks/post-receive");
        let mode = fs::metadata(&hook).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "{} has mode {mode:o}", hook.display());
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
