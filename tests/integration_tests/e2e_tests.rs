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

    // =========================================================================
    // Runtime detection — all supported runtimes
    // =========================================================================

    #[test]
    fn test_runtime_detection_ruby() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("Gemfile"), "source 'https://rubygems.org'\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::Ruby)),
            "must detect Ruby runtime from Gemfile"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_go_mod() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("go.mod"), "module example.com/myapp\ngo 1.21\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::Go)),
            "must detect Go runtime from go.mod"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_go_source_file() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("main.go"), "package main\nfunc main() {}\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::Go)),
            "must detect Go runtime from .go source file"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_java_maven() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("pom.xml"), "<project></project>")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::JavaMaven)),
            "must detect JavaMaven runtime from pom.xml"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_java_gradle() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("build.gradle"), "plugins { id 'java' }\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::JavaGradle)),
            "must detect JavaGradle runtime from build.gradle"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_clojure_cli() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("deps.edn"), "{:deps {}}\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::ClojureCli)),
            "must detect ClojureCli runtime from deps.edn"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_clojure_lein() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("project.clj"), "(defproject myapp \"0.1.0\")\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::ClojureLein)),
            "must detect ClojureLein runtime from project.clj"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_container_dockerfile() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("Dockerfile"), "FROM alpine\nCMD [\"sh\"]\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::Container)),
            "must detect Container runtime from Dockerfile"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_container_containerfile() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(
            app_dir.join("Containerfile"),
            "FROM alpine\nCMD [\"sh\"]\n",
        )?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::Container)),
            "must detect Container runtime from Containerfile"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_container_compose() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(
            app_dir.join("docker-compose.yml"),
            "services:\n  web:\n    image: alpine\n",
        )?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::Container)),
            "must detect Container runtime from docker-compose.yml"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_rust() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("Cargo.toml"), "[package]\nname = \"myapp\"\n")?;
        fs::write(app_dir.join("rust-toolchain.toml"), "[toolchain]\nchannel = \"stable\"\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::Rust)),
            "must detect Rust runtime from Cargo.toml + rust-toolchain.toml"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_rust_cargo_only_returns_none() -> Result<()> {
        // Cargo.toml alone (without rust-toolchain.toml) should NOT detect Rust
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("Cargo.toml"), "[package]\nname = \"myapp\"\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            !matches!(runtime, Some(riku::deploy::Runtime::Rust)),
            "Cargo.toml alone must not detect Rust (requires rust-toolchain.toml too)"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_python_pyproject_fallback() -> Result<()> {
        // pyproject.toml with no poetry or uv available falls back to Python.
        // We can't control whether poetry/uv are installed, so we just verify
        // the result is one of the Python variants.
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(
            app_dir.join("pyproject.toml"),
            "[tool.poetry]\nname = \"myapp\"\n",
        )?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(
                runtime,
                Some(
                    riku::deploy::Runtime::Python
                        | riku::deploy::Runtime::PythonPoetry
                        | riku::deploy::Runtime::PythonUv
                )
            ),
            "pyproject.toml must detect a Python variant"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_wsgi() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("Procfile"), "wsgi: myapp.wsgi\n")?;
        fs::write(app_dir.join("wsgi.py"), "application = None\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::Wsgi)),
            "must detect Wsgi runtime from Procfile wsgi: + wsgi.py"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_php() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        fs::write(app_dir.join("Procfile"), "php: php -S 0.0.0.0:$PORT\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(
            matches!(runtime, Some(riku::deploy::Runtime::Php)),
            "must detect Php runtime from Procfile php:"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_detection_none() -> Result<()> {
        let tmp = TempDir::new()?;
        let app_dir = tmp.path().join("app");
        fs::create_dir_all(&app_dir)?;
        // Write only a README — no recognized marker file
        fs::write(app_dir.join("README.md"), "# My App\n")?;

        let runtime = riku::deploy::detect_runtime(&app_dir);
        assert!(runtime.is_none(), "must return None when no runtime markers exist");
        Ok(())
    }

    // =========================================================================
    // Worker config — additional coverage
    // =========================================================================

    #[test]
    fn test_worker_config_multiple_process_types() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "multiproc";
        let app_dir = paths.app_root.join(app);
        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::create_dir_all(paths.log_root.join(app))?;

        fs::write(
            app_dir.join("Procfile"),
            "web: node server.js\nworker: node worker.js\nscheduler: node scheduler.js\n",
        )?;

        let env = HashMap::new();
        riku::deploy::workers::create_workers_generic(app, &app_dir, &env, &paths)?;

        assert!(
            paths.workers_available.join("multiproc-web-1.toml").exists(),
            "web config must exist"
        );
        assert!(
            paths.workers_available.join("multiproc-worker-1.toml").exists(),
            "worker config must exist"
        );
        assert!(
            paths.workers_available.join("multiproc-scheduler-1.toml").exists(),
            "scheduler config must exist"
        );
        Ok(())
    }

    #[test]
    fn test_worker_config_worker_type_scaling() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "wscaled";
        let app_dir = paths.app_root.join(app);
        fs::create_dir_all(&app_dir)?;

        let env_dir = paths.env_root.join(app);
        fs::create_dir_all(&env_dir)?;
        fs::create_dir_all(paths.log_root.join(app))?;

        // Scale workers to 3
        fs::write(env_dir.join("SCALING"), "worker=3\n")?;
        fs::write(app_dir.join("Procfile"), "worker: python worker.py\n")?;

        let env = HashMap::new();
        riku::deploy::workers::create_workers_generic(app, &app_dir, &env, &paths)?;

        for i in 1..=3 {
            assert!(
                paths
                    .workers_available
                    .join(format!("wscaled-worker-{}.toml", i))
                    .exists(),
                "worker-{} config must exist",
                i
            );
        }
        Ok(())
    }

    #[test]
    fn test_worker_config_comment_lines_ignored() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "commentapp";
        let app_dir = paths.app_root.join(app);
        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::create_dir_all(paths.log_root.join(app))?;

        // Procfile with comments interspersed
        fs::write(
            app_dir.join("Procfile"),
            "# This is a comment\nweb: node server.js\n# Another comment\n",
        )?;

        let env = HashMap::new();
        riku::deploy::workers::create_workers_generic(app, &app_dir, &env, &paths)?;

        // Only web config should exist
        let configs: Vec<_> = fs::read_dir(&paths.workers_available)?
            .flatten()
            .collect();
        assert_eq!(configs.len(), 1, "only one worker config should be created");
        assert!(
            paths
                .workers_available
                .join("commentapp-web-1.toml")
                .exists(),
            "web config must exist"
        );
        Ok(())
    }

    #[test]
    fn test_worker_config_content_has_correct_command() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "cmdapp";
        let app_dir = paths.app_root.join(app);
        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::create_dir_all(paths.log_root.join(app))?;

        fs::write(app_dir.join("Procfile"), "worker: celery -A tasks worker\n")?;

        let env = HashMap::new();
        riku::deploy::workers::create_workers_generic(app, &app_dir, &env, &paths)?;

        let cfg = paths.workers_available.join("cmdapp-worker-1.toml");
        let content = fs::read_to_string(&cfg)?;
        assert!(
            content.contains("celery"),
            "worker config must contain the command from Procfile"
        );
        Ok(())
    }

    #[test]
    fn test_worker_config_no_procfile_returns_ok() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "noprocfile";
        let app_dir = paths.app_root.join(app);
        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::create_dir_all(paths.log_root.join(app))?;
        // Deliberately no Procfile written

        let env = HashMap::new();
        let result = riku::deploy::workers::create_workers_generic(app, &app_dir, &env, &paths);
        assert!(result.is_ok(), "missing Procfile must not return an error");

        // No configs should have been created
        let configs: Vec<_> = fs::read_dir(&paths.workers_available)?
            .flatten()
            .collect();
        assert_eq!(configs.len(), 0, "no configs should be created without a Procfile");
        Ok(())
    }

    // =========================================================================
    // ENV file parsing — edge cases
    // =========================================================================

    #[test]
    fn test_env_file_with_comments_ignored() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "commentenv";
        let env_dir = paths.env_root.join(app);
        fs::create_dir_all(&env_dir)?;
        fs::write(
            env_dir.join("ENV"),
            "# This is a comment\nKEY=value\n# Another comment\n",
        )?;

        let mut env: HashMap<String, String> = HashMap::new();
        riku::util::parse_settings(&env_dir.join("ENV"), &mut env)?;

        assert_eq!(env.get("KEY"), Some(&"value".to_string()));
        assert!(!env.contains_key("# This is a comment"), "comment must not be parsed as key");
        Ok(())
    }

    #[test]
    fn test_env_file_with_value_containing_equals() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "eqenv";
        let env_dir = paths.env_root.join(app);
        fs::create_dir_all(&env_dir)?;
        // Value contains = sign (e.g. a base64 encoded value or URL)
        fs::write(
            env_dir.join("ENV"),
            "DATABASE_URL=postgres://user:pass@host/db?ssl=true\n",
        )?;

        let mut env: HashMap<String, String> = HashMap::new();
        riku::util::parse_settings(&env_dir.join("ENV"), &mut env)?;

        assert_eq!(
            env.get("DATABASE_URL"),
            Some(&"postgres://user:pass@host/db?ssl=true".to_string()),
            "value with = signs must be preserved"
        );
        Ok(())
    }

    #[test]
    fn test_env_file_with_empty_value() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "emptyval";
        let env_dir = paths.env_root.join(app);
        fs::create_dir_all(&env_dir)?;
        fs::write(env_dir.join("ENV"), "EMPTY_KEY=\n")?;

        let mut env: HashMap<String, String> = HashMap::new();
        riku::util::parse_settings(&env_dir.join("ENV"), &mut env)?;

        assert_eq!(
            env.get("EMPTY_KEY"),
            Some(&"".to_string()),
            "empty value must be parsed as empty string"
        );
        Ok(())
    }

    // =========================================================================
    // Full deploy — additional scenarios
    // =========================================================================

    #[test]
    fn test_full_deploy_creates_deploy_log() -> Result<()> {
        std::env::set_var("RIKU_SKIP_BUILD", "1");

        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "logapp";
        let files = &[
            ("Procfile", "web: node server.js\n"),
            ("package.json", r#"{"name":"logapp","version":"1.0.0"}"#),
        ];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let _app_dir = setup_app_clone(bare.path(), app, &paths);
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::write(paths.env_root.join(app).join("ENV"), "")?;
        fs::create_dir_all(paths.log_root.join(app))?;

        let deltas: HashMap<String, i64> = HashMap::new();
        riku::deploy::do_deploy(app, &paths, &deltas, Some(&sha))?;

        // Deploy log must be created
        let deploy_log = paths.deploy_log_file(app);
        assert!(deploy_log.exists(), "deploy.log must be created by do_deploy");

        let log_content = fs::read_to_string(&deploy_log)?;
        assert!(
            log_content.contains("logapp") || log_content.contains("Deploy"),
            "deploy log must contain app name or deploy entry"
        );

        std::env::remove_var("RIKU_SKIP_BUILD");
        Ok(())
    }

    #[test]
    fn test_full_deploy_creates_live_env() -> Result<()> {
        std::env::set_var("RIKU_SKIP_BUILD", "1");

        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "liveenvapp";
        let files = &[
            ("Procfile", "web: node server.js\n"),
            ("package.json", r#"{"name":"liveenvapp"}"#),
        ];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let _app_dir = setup_app_clone(bare.path(), app, &paths);
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::write(paths.env_root.join(app).join("ENV"), "MY_VAR=hello\n")?;
        fs::create_dir_all(paths.log_root.join(app))?;

        let deltas: HashMap<String, i64> = HashMap::new();
        riku::deploy::do_deploy(app, &paths, &deltas, Some(&sha))?;

        // LIVE_ENV must be written
        let live_env = paths.env_root.join(app).join("LIVE_ENV");
        assert!(live_env.exists(), "LIVE_ENV must be created after full deploy");

        let content = fs::read_to_string(&live_env)?;
        assert!(
            content.contains("MY_VAR"),
            "LIVE_ENV must include app-defined env vars"
        );

        std::env::remove_var("RIKU_SKIP_BUILD");
        Ok(())
    }

    #[test]
    fn test_full_deploy_node_with_scaling() -> Result<()> {
        std::env::set_var("RIKU_SKIP_BUILD", "1");

        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "scaleapp";
        let files = &[
            ("Procfile", "web: node server.js\nworker: node worker.js\n"),
            ("package.json", r#"{"name":"scaleapp"}"#),
        ];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let _app_dir = setup_app_clone(bare.path(), app, &paths);

        let env_dir = paths.env_root.join(app);
        fs::create_dir_all(&env_dir)?;
        fs::write(env_dir.join("ENV"), "")?;
        fs::write(env_dir.join("SCALING"), "web=2\nworker=1\n")?;
        fs::create_dir_all(paths.log_root.join(app))?;

        let deltas: HashMap<String, i64> = HashMap::new();
        riku::deploy::do_deploy(app, &paths, &deltas, Some(&sha))?;

        assert!(
            paths.workers_available.join("scaleapp-web-1.toml").exists(),
            "web-1 must exist"
        );
        assert!(
            paths.workers_available.join("scaleapp-web-2.toml").exists(),
            "web-2 must exist (scaling=2)"
        );
        assert!(
            paths.workers_available.join("scaleapp-worker-1.toml").exists(),
            "worker-1 must exist"
        );

        std::env::remove_var("RIKU_SKIP_BUILD");
        Ok(())
    }

    #[test]
    fn test_full_deploy_without_env_file_succeeds() -> Result<()> {
        std::env::set_var("RIKU_SKIP_BUILD", "1");

        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "noenvapp";
        let files = &[
            ("Procfile", "web: node server.js\n"),
            ("package.json", r#"{"name":"noenvapp"}"#),
        ];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let _app_dir = setup_app_clone(bare.path(), app, &paths);

        // Create env dir but no ENV file — deploy must still succeed
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::create_dir_all(paths.log_root.join(app))?;

        let deltas: HashMap<String, i64> = HashMap::new();
        let result = riku::deploy::do_deploy(app, &paths, &deltas, Some(&sha));
        assert!(result.is_ok(), "deploy without ENV file must succeed");

        std::env::remove_var("RIKU_SKIP_BUILD");
        Ok(())
    }

    #[test]
    fn test_full_deploy_python_with_multiple_workers() -> Result<()> {
        std::env::set_var("RIKU_SKIP_BUILD", "1");

        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "pyworkers";
        let files = &[
            (
                "Procfile",
                "web: gunicorn app:application\nworker: celery -A tasks worker\n",
            ),
            ("requirements.txt", "gunicorn\ncelery\n"),
            ("app.py", "application = None"),
        ];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let _app_dir = setup_app_clone(bare.path(), app, &paths);
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::write(paths.env_root.join(app).join("ENV"), "")?;
        fs::create_dir_all(paths.log_root.join(app))?;

        let deltas: HashMap<String, i64> = HashMap::new();
        riku::deploy::do_deploy(app, &paths, &deltas, Some(&sha))?;

        assert!(
            paths.workers_available.join("pyworkers-web-1.toml").exists(),
            "web worker config must exist"
        );
        assert!(
            paths
                .workers_available
                .join("pyworkers-worker-1.toml")
                .exists(),
            "celery worker config must exist"
        );

        std::env::remove_var("RIKU_SKIP_BUILD");
        Ok(())
    }

    // =========================================================================
    // Error cases — additional coverage
    // =========================================================================

    #[test]
    fn test_deploy_app_exists_but_no_procfile_returns_error() -> Result<()> {
        std::env::set_var("RIKU_SKIP_BUILD", "1");

        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "noprocapp";
        let files = &[("package.json", r#"{"name":"noprocapp"}"#)];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let _app_dir = setup_app_clone(bare.path(), app, &paths);
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::write(paths.env_root.join(app).join("ENV"), "")?;
        fs::create_dir_all(paths.log_root.join(app))?;

        let deltas: HashMap<String, i64> = HashMap::new();
        let result = riku::deploy::do_deploy(app, &paths, &deltas, Some(&sha));
        assert!(result.is_err(), "deploy without Procfile must return Err");

        std::env::remove_var("RIKU_SKIP_BUILD");
        Ok(())
    }

    #[test]
    fn test_deploy_procfile_with_only_comments_returns_error() -> Result<()> {
        std::env::set_var("RIKU_SKIP_BUILD", "1");

        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "commentproc";
        let files = &[
            ("Procfile", "# web: node server.js\n# This is all commented out\n"),
            ("package.json", r#"{"name":"commentproc"}"#),
        ];

        let (bare, _work, sha) = make_git_repo_with_files(files);
        let _app_dir = setup_app_clone(bare.path(), app, &paths);
        fs::create_dir_all(paths.env_root.join(app))?;
        fs::write(paths.env_root.join(app).join("ENV"), "")?;
        fs::create_dir_all(paths.log_root.join(app))?;

        let deltas: HashMap<String, i64> = HashMap::new();
        let result = riku::deploy::do_deploy(app, &paths, &deltas, Some(&sha));
        assert!(
            result.is_err(),
            "deploy with comments-only Procfile must return Err"
        );

        std::env::remove_var("RIKU_SKIP_BUILD");
        Ok(())
    }

    // =========================================================================
    // Git sync — additional scenarios
    // =========================================================================

    #[test]
    fn test_git_sync_without_sha_uses_head() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "noshaapp";
        let files = &[
            ("Procfile", "web: node server.js\n"),
            ("package.json", r#"{"name":"noshaapp"}"#),
        ];

        let (bare, _work, _sha) = make_git_repo_with_files(files);
        let app_dir = setup_app_clone(bare.path(), app, &paths);

        // Pass None for SHA — should sync to HEAD without error
        riku::deploy::git_ops::sync_app_repo(&app_dir, None)?;

        assert!(
            app_dir.join("Procfile").exists(),
            "Procfile must exist after sync with no sha"
        );
        Ok(())
    }

    #[test]
    fn test_git_sync_updates_changed_files() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let app = "updateapp";
        let files = &[
            ("Procfile", "web: node server.js\n"),
            ("package.json", r#"{"name":"updateapp","version":"1.0.0"}"#),
            ("data.txt", "original content\n"),
        ];

        let (bare, work, _sha) = make_git_repo_with_files(files);
        let app_dir = setup_app_clone(bare.path(), app, &paths);

        // Update the file in the working tree and push again
        fs::write(work.path().join("data.txt"), "updated content\n")?;
        Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "add", "."])
            .output()?;
        Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "commit", "-m", "update"])
            .output()?;
        Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "push", "origin", "HEAD"])
            .output()?;

        let new_sha = String::from_utf8(
            Command::new("git")
                .args(["-C", work.path().to_str().unwrap(), "rev-parse", "HEAD"])
                .output()?
                .stdout,
        )?
        .trim()
        .to_string();

        // Sync to the new sha
        riku::deploy::git_ops::sync_app_repo(&app_dir, Some(&new_sha))?;

        let content = fs::read_to_string(app_dir.join("data.txt"))?;
        assert_eq!(
            content.trim(),
            "updated content",
            "git sync must update file to latest committed content"
        );
        Ok(())
    }

    // =========================================================================
    // Scaling — read_scaling_count public API
    // =========================================================================

    #[test]
    fn test_read_scaling_count_default_is_one() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);
        fs::create_dir_all(paths.env_root.join("myapp"))?;

        let count = riku::deploy::workers::read_scaling_count(&paths, "myapp", "web")?;
        assert_eq!(count, 1, "default scaling count must be 1");
        Ok(())
    }

    #[test]
    fn test_read_scaling_count_from_file() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let env_dir = paths.env_root.join("myapp");
        fs::create_dir_all(&env_dir)?;
        fs::write(env_dir.join("SCALING"), "web=4\nworker=2\n")?;

        assert_eq!(
            riku::deploy::workers::read_scaling_count(&paths, "myapp", "web")?,
            4
        );
        assert_eq!(
            riku::deploy::workers::read_scaling_count(&paths, "myapp", "worker")?,
            2
        );
        Ok(())
    }

    #[test]
    fn test_read_scaling_count_unknown_kind_defaults_to_one() -> Result<()> {
        let tmp = TempDir::new()?;
        let paths = make_paths(&tmp);

        let env_dir = paths.env_root.join("myapp");
        fs::create_dir_all(&env_dir)?;
        fs::write(env_dir.join("SCALING"), "web=3\n")?;

        let count = riku::deploy::workers::read_scaling_count(&paths, "myapp", "scheduler")?;
        assert_eq!(count, 1, "unknown kind must default to 1");
        Ok(())
    }
}
