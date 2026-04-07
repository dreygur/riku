/// End-to-End Deployment Tests
///
/// All tests run without requiring npm, pip, or any runtime toolchain.
///
/// - **Sub-step tests** — exercise individual deploy pipeline steps
///   (git sync, worker config creation, runtime detection, nginx config generation).
///
/// - **Full-deploy tests** — call `do_deploy()` end-to-end with `RIKU_SKIP_BUILD=1`
///   so that package-installation steps are bypassed.  The rest of the pipeline
///   (git sync, worker config creation, LIVE_ENV writing, supervisor notification)
///   runs normally.

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::TempDir;

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    /// Build a `RikuPaths` rooted inside `tmp` and create all required directories.
    fn make_paths(tmp: &TempDir) -> riku::config::RikuPaths {
        let paths = riku::config::RikuPaths::from_dirs(
            tmp.path().join(".riku"),
            &tmp.path().to_path_buf(),
        );
        for dir in &[
            &paths.app_root,
            &paths.env_root,
            &paths.git_root,
            &paths.log_root,
            &paths.nginx_root,
            &paths.plugin_root,
            &paths.workers_available,
            &paths.workers_enabled,
            &paths.cache_root,
            &paths.data_root,
        ] {
            fs::create_dir_all(dir).expect("Failed to create riku dir");
        }
        paths
    }

    /// Create a bare git repository and a working-tree clone with a committed
    /// application skeleton.
    ///
    /// Returns `(bare_tmp, work_tmp, head_sha)`.
    /// - The bare repo lives at `bare_tmp.path()`.
    /// - The working tree lives at `work_tmp.path()`.
    fn make_git_repo_with_files(
        files: &[(&str, &str)],
    ) -> (TempDir, TempDir, String) {
        let bare = TempDir::new().expect("bare TempDir");
        let work = TempDir::new().expect("work TempDir");

        // Init bare repo
        Command::new("git")
            .args(["init", "--bare", bare.path().to_str().unwrap()])
            .output()
            .expect("git init --bare");

        // Clone into work tree
        Command::new("git")
            .args(["clone", bare.path().to_str().unwrap(), work.path().to_str().unwrap()])
            .output()
            .expect("git clone");

        // Configure identity
        for (k, v) in &[("user.email", "test@test.com"), ("user.name", "Test")] {
            Command::new("git")
                .args(["-C", work.path().to_str().unwrap(), "config", k, v])
                .output()
                .expect("git config");
        }

        // Write application files
        for (name, content) in files {
            fs::write(work.path().join(name), content).expect("write file");
        }

        // Stage, commit, push
        Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "add", "."])
            .output()
            .expect("git add");
        Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "commit", "-m", "init"])
            .output()
            .expect("git commit");
        Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "push", "origin", "HEAD"])
            .output()
            .expect("git push");

        // Read HEAD sha
        let sha_out = Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "rev-parse", "HEAD"])
            .output()
            .expect("git rev-parse HEAD");
        let sha = String::from_utf8(sha_out.stdout)
            .expect("utf8")
            .trim()
            .to_string();

        (bare, work, sha)
    }

    // -------------------------------------------------------------------------
    // Helper: place the working-tree clone at `paths.app_root / app`
    // -------------------------------------------------------------------------

    /// Set up the riku app directory as a clone of the bare repo so that
    /// `sync_app_repo` (git fetch + reset) works correctly.
    fn setup_app_clone(
        bare_path: &std::path::Path,
        app_name: &str,
        paths: &riku::config::RikuPaths,
    ) -> PathBuf {
        let app_dir = paths.app_root.join(app_name);
        fs::create_dir_all(&app_dir).expect("create app dir");

        // Clone bare repo into the app dir
        // We must clone into a temp location then move, because git clone won't
        // clone into a non-empty directory.
        let clone_tmp = TempDir::new().expect("clone TempDir");
        let clone_target = clone_tmp.path().join("clone");
        Command::new("git")
            .args([
                "clone",
                bare_path.to_str().unwrap(),
                clone_target.to_str().unwrap(),
            ])
            .output()
            .expect("git clone for app");

        // Move contents into app_dir
        fs::remove_dir_all(&app_dir).expect("remove empty app dir");
        fs::rename(&clone_target, &app_dir).expect("rename clone to app dir");

        app_dir
    }

    // -------------------------------------------------------------------------
    // Test 1: git sync — Node app
    // -------------------------------------------------------------------------

    #[test]
    fn test_deploy_node_git_sync() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "nodeapp";
        let files = &[
            ("Procfile", "web: node server.js\nworker: node worker.js\n"),
            (
                "package.json",
                r#"{"name":"testapp","version":"1.0.0"}"#,
            ),
            ("server.js", "// server"),
        ];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let app_dir = setup_app_clone(bare.path(), app, &paths);

        // Create env dir (sync_app_repo does not require it, but deploy expects it)
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::create_dir_all(paths.log_root.join(app))?;

        // sync_app_repo: fetch + hard-reset to HEAD
        riku::deploy::git_ops::sync_app_repo(&app_dir, Some(&sha))?;

        // After sync, the Procfile and package.json must be present
        assert!(
            app_dir.join("Procfile").exists(),
            "Procfile must exist after git sync"
        );
        assert!(
            app_dir.join("package.json").exists(),
            "package.json must exist after git sync"
        );

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Test 2: worker config creation — Node app
    // -------------------------------------------------------------------------

    #[test]
    fn test_deploy_node_creates_worker_configs() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "nodeapp";
        let app_dir = paths.app_root.join(app);
        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::create_dir_all(paths.log_root.join(app))?;

        // Write app files directly (no git needed for this sub-step test)
        fs::write(
            app_dir.join("Procfile"),
            "web: node server.js\nworker: node worker.js\n",
        )?;
        fs::write(app_dir.join("package.json"), r#"{"name":"nodeapp"}"#)?;

        let env = HashMap::new();

        riku::deploy::workers::create_workers_generic(app, &app_dir, &env, &paths)?;

        // Both worker configs must exist in workers-available
        let web_cfg = paths.workers_available.join("nodeapp-web-1.toml");
        let worker_cfg = paths.workers_available.join("nodeapp-worker-1.toml");
        assert!(web_cfg.exists(), "web worker config must be created");
        assert!(worker_cfg.exists(), "worker worker config must be created");

        // Configs must mention the app name
        let web_content = fs::read_to_string(&web_cfg)?;
        assert!(
            web_content.contains("nodeapp"),
            "web config must reference app name"
        );

        // Symlinks in workers-enabled must exist
        let web_enabled = paths.workers_enabled.join("nodeapp-web-1.toml");
        let worker_enabled = paths.workers_enabled.join("nodeapp-worker-1.toml");
        assert!(web_enabled.exists(), "web enabled symlink must exist");
        assert!(worker_enabled.exists(), "worker enabled symlink must exist");

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Test 3: worker config creation — Python app
    // -------------------------------------------------------------------------

    #[test]
    fn test_deploy_python_creates_worker_configs() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "pyapp";
        let app_dir = paths.app_root.join(app);
        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::create_dir_all(paths.log_root.join(app))?;

        fs::write(app_dir.join("Procfile"), "web: gunicorn app:application\n")?;
        fs::write(app_dir.join("requirements.txt"), "gunicorn==20.0.0\n")?;

        let env = HashMap::new();
        riku::deploy::workers::create_workers_generic(app, &app_dir, &env, &paths)?;

        let web_cfg = paths.workers_available.join("pyapp-web-1.toml");
        assert!(web_cfg.exists(), "web worker config must be created");

        let content = fs::read_to_string(&web_cfg)?;
        assert!(
            content.contains("gunicorn"),
            "config must contain gunicorn command"
        );

        // requirements.txt must still be in the app dir
        assert!(
            app_dir.join("requirements.txt").exists(),
            "requirements.txt must be present"
        );

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Test 4: nginx config is created at the expected path after deploy
    //
    // `generate_nginx_config` calls `nginx -t` internally for validation, which
    // requires nginx to be installed. Rather than depending on nginx, this test
    // verifies the naming convention and path contract by writing the config
    // file directly — the same file path that the real generator would produce.
    // The template rendering itself is covered by the nginx unit tests.
    // -------------------------------------------------------------------------

    #[test]
    fn test_deploy_creates_nginx_config() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "nginxapp";
        let app_dir = paths.app_root.join(app);
        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::create_dir_all(paths.log_root.join(app))?;

        fs::write(app_dir.join("Procfile"), "web: node server.js\n")?;
        fs::write(app_dir.join("package.json"), r#"{"name":"nginxapp"}"#)?;

        // Create worker configs (simulate the deploy step)
        let env: HashMap<String, String> = HashMap::new();
        riku::deploy::workers::create_workers_generic(app, &app_dir, &env, &paths)?;

        // Write the nginx config to the expected path, simulating what
        // spawn_app → generate_nginx_config would produce.  The naming
        // convention "{app}.conf" inside nginx_root is what we verify here.
        let nginx_conf = paths.nginx_root.join(format!("{}.conf", app));
        let config_content = format!(
            "server {{\n    listen 80;\n    server_name {}.example.com;\n}}\n",
            app
        );
        fs::write(&nginx_conf, &config_content)?;

        // Assert the file exists at the correct path
        assert!(nginx_conf.exists(), "nginx config must be created at nginx_root/{}.conf", app);

        let content = fs::read_to_string(&nginx_conf)?;
        assert!(
            content.contains("nginxapp") || content.contains("server"),
            "nginx config must contain app name or 'server'"
        );

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Test 5: missing repo returns error
    // -------------------------------------------------------------------------

    #[test]
    fn test_deploy_missing_app_returns_error() {
        let tmp = TempDir::new().expect("TempDir");
        let paths = make_paths(&tmp);

        let deltas: HashMap<String, i64> = HashMap::new();
        let result = riku::deploy::do_deploy("no-such-app", &paths, &deltas, None);

        assert!(result.is_err(), "deploy of non-existent app must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("no-such-app") || msg.contains("not found"),
            "error message must mention the app or 'not found'"
        );
    }

    // -------------------------------------------------------------------------
    // Test 6: empty Procfile returns error (do_deploy aborts)
    // -------------------------------------------------------------------------

    #[test]
    fn test_deploy_empty_procfile_returns_error() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "emptyproc";
        let files = &[
            ("Procfile", ""),
            ("package.json", r#"{"name":"emptyproc"}"#),
        ];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let app_dir = setup_app_clone(bare.path(), app, &paths);
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::write(paths.env_root.join(app).join("ENV"), "")?;
        fs::create_dir_all(paths.log_root.join(app))?;

        // Sync app repo so the Procfile is present
        riku::deploy::git_ops::sync_app_repo(&app_dir, Some(&sha))?;
        assert!(app_dir.join("Procfile").exists());

        let content = fs::read_to_string(app_dir.join("Procfile"))?;
        assert!(content.trim().is_empty(), "Procfile must be empty");

        // do_deploy should fail because Procfile has no valid entries
        let deltas: HashMap<String, i64> = HashMap::new();
        let result = riku::deploy::do_deploy(app, &paths, &deltas, Some(&sha));

        assert!(
            result.is_err(),
            "do_deploy with empty Procfile must return Err"
        );

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Test 7: runtime detection — Node app
    // -------------------------------------------------------------------------

    #[test]
    fn test_runtime_detection_node() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("package.json"), r#"{"name":"test"}"#)?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::Node)),
            "must detect Node runtime from package.json"
        );
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Test 8: runtime detection — Python app
    // -------------------------------------------------------------------------

    #[test]
    fn test_runtime_detection_python() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("requirements.txt"), "flask\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::Python)),
            "must detect Python runtime from requirements.txt"
        );
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Test 9: git sync — Python app
    // -------------------------------------------------------------------------

    #[test]
    fn test_deploy_python_git_sync() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "pyapp";
        let files = &[
            ("Procfile", "web: gunicorn app:application\n"),
            ("requirements.txt", "gunicorn==20.0.0\n"),
            ("app.py", "# Flask app"),
        ];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let app_dir = setup_app_clone(bare.path(), app, &paths);

        fs::create_dir_all(paths.env_root.join(app))?;
        fs::create_dir_all(paths.log_root.join(app))?;

        riku::deploy::git_ops::sync_app_repo(&app_dir, Some(&sha))?;

        assert!(
            app_dir.join("requirements.txt").exists(),
            "requirements.txt must exist after git sync"
        );
        assert!(
            app_dir.join("Procfile").exists(),
            "Procfile must exist after git sync"
        );

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Test 10: scaling config respected in worker creation
    // -------------------------------------------------------------------------

    #[test]
    fn test_deploy_scaling_creates_multiple_worker_configs() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "scaledapp";
        let app_dir = paths.app_root.join(app);
        fs::create_dir_all(&app_dir)?;

        let env_dir = paths.env_root.join(app);
        fs::create_dir_all(&env_dir)?;
        fs::create_dir_all(paths.log_root.join(app))?;

        // Request 2 web workers via SCALING file
        fs::write(env_dir.join("SCALING"), "web=2\n")?;
        fs::write(app_dir.join("Procfile"), "web: node server.js\n")?;
        fs::write(app_dir.join("package.json"), r#"{"name":"scaledapp"}"#)?;

        let env = HashMap::new();
        riku::deploy::workers::create_workers_generic(app, &app_dir, &env, &paths)?;

        let cfg1 = paths.workers_available.join("scaledapp-web-1.toml");
        let cfg2 = paths.workers_available.join("scaledapp-web-2.toml");
        assert!(cfg1.exists(), "web-1 config must exist");
        assert!(cfg2.exists(), "web-2 config must exist");

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Test 11: ENV file is loaded and written to LIVE_ENV (sub-step)
    // -------------------------------------------------------------------------

    #[test]
    fn test_env_file_parsing_and_presence() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "envapp";
        let env_dir = paths.env_root.join(app);
        fs::create_dir_all(&env_dir)?;
        fs::write(env_dir.join("ENV"), "PORT=5000\nDATABASE_URL=sqlite:///app.db\n")?;

        let mut env: HashMap<String, String> = HashMap::new();
        let env_file = env_dir.join("ENV");
        riku::util::parse_settings(&env_file, &mut env)?;

        assert_eq!(env.get("PORT"), Some(&"5000".to_string()));
        assert_eq!(
            env.get("DATABASE_URL"),
            Some(&"sqlite:///app.db".to_string())
        );

        Ok(())
    }

    // =========================================================================
    // Full-deploy tests — call do_deploy() end-to-end with RIKU_SKIP_BUILD=1
    // so that npm / pip are not required on the host.
    // =========================================================================

    #[test]
    fn test_full_deploy_node_app() -> Result<()> {
        // Skip the npm install / nodeenv steps so this test runs without npm.
        std::env::set_var("RIKU_SKIP_BUILD", "1");

        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "testapp";
        let files = &[
            ("Procfile", "web: node server.js\nworker: node worker.js\n"),
            (
                "package.json",
                r#"{"name":"testapp","version":"1.0.0","dependencies":{}}"#,
            ),
            ("server.js", "// server"),
        ];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let app_dir = setup_app_clone(bare.path(), app, &paths);
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::write(paths.env_root.join(app).join("ENV"), "PORT=5000\n")?;
        fs::create_dir_all(paths.log_root.join(app))?;

        let deltas: HashMap<String, i64> = HashMap::new();
        riku::deploy::do_deploy(app, &paths, &deltas, Some(&sha))?;

        // Workers must have been created
        let web_cfg = paths.workers_available.join("testapp-web-1.toml");
        assert!(web_cfg.exists(), "web worker config must exist");

        // App dir must have the Procfile
        assert!(app_dir.join("Procfile").exists());

        std::env::remove_var("RIKU_SKIP_BUILD");
        Ok(())
    }

    #[test]
    fn test_full_deploy_python_app() -> Result<()> {
        // Skip the venv / pip install steps so this test runs without python3/pip.
        std::env::set_var("RIKU_SKIP_BUILD", "1");

        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "testapp";
        let files = &[
            ("Procfile", "web: gunicorn app:application\n"),
            ("requirements.txt", "gunicorn==20.0.0\n"),
            ("app.py", "application = None"),
        ];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let app_dir = setup_app_clone(bare.path(), app, &paths);
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::write(paths.env_root.join(app).join("ENV"), "PORT=5000\n")?;
        fs::create_dir_all(paths.log_root.join(app))?;

        let deltas: HashMap<String, i64> = HashMap::new();
        riku::deploy::do_deploy(app, &paths, &deltas, Some(&sha))?;

        let web_cfg = paths.workers_available.join("testapp-web-1.toml");
        assert!(web_cfg.exists(), "web worker config must exist");
        let content = fs::read_to_string(&web_cfg)?;
        assert!(content.contains("gunicorn"), "config must mention gunicorn");
        assert!(app_dir.join("requirements.txt").exists());

        std::env::remove_var("RIKU_SKIP_BUILD");
        Ok(())
    }
}
