//! Convenience macros for deploy steps.
//!
//! These macros reduce boilerplate in runtime-specific deployers that need to
//! allocate a TCP port, write NGINX port-map variables, and persist worker
//! config TOML files.

/// Set up PORT/SOCKET/NGINX_PORTMAP env vars for a web worker and persist them
/// to the app ENV file.  Expands to the allocated `u16` port number.
///
/// Usage (inside a `Result`-returning function):
/// ```ignore
/// let port = setup_web_port!(worker_env, app, paths);
/// ```
#[macro_export]
macro_rules! setup_web_port {
    ($worker_env:expr, $app:expr, $paths:expr) => {{
        use $crate::util::get_free_port;
        let port = get_free_port("127.0.0.1")?;
        $worker_env.insert("PORT".to_string(), port.to_string());

        let socket_path = $paths.nginx_root.join(format!("{}.sock", $app));
        $worker_env.insert(
            "SOCKET".to_string(),
            socket_path.to_string_lossy().to_string(),
        );

        $worker_env.insert("NGINX_PORTMAP".to_string(), "true".to_string());
        $worker_env.insert("NGINX_INTERNAL_PORT".to_string(), port.to_string());
        $worker_env.insert("NGINX_EXTERNAL_PORT".to_string(), "80".to_string());

        let env_dir = $paths.env_root.join($app);
        std::fs::create_dir_all(&env_dir)?;
        let env_file = env_dir.join("ENV");
        let mut env_content = if env_file.exists() {
            std::fs::read_to_string(&env_file)?
        } else {
            String::new()
        };
        if !env_content.contains("NGINX_PORTMAP") {
            env_content.push_str("NGINX_PORTMAP=true\n");
            env_content.push_str(&format!("NGINX_INTERNAL_PORT={}\n", port));
            env_content.push_str("NGINX_EXTERNAL_PORT=80\n");
            std::fs::write(&env_file, &env_content)?;
        }
        port
    }};
}

/// Write a worker config TOML to `workers_available/` and symlink it into
/// `workers_enabled/`.  Emits the standard "Created worker config" message.
///
/// Usage (inside a `Result`-returning function):
/// ```ignore
/// write_worker_config!(app, kind, &final_command, ordinal, worker_env, app_path, paths);
/// ```
#[macro_export]
macro_rules! write_worker_config {
    ($app:expr, $kind:expr, $command:expr, $ordinal:expr, $worker_env:expr, $app_path:expr, $paths:expr) => {{
        use $crate::supervisor::config::create_worker_config;
        use $crate::util::echo;
        let worker_config = create_worker_config(
            $app,
            $kind,
            $command,
            $ordinal,
            $worker_env,
            &$app_path.to_string_lossy(),
            &$paths
                .log_root
                .join($app)
                .join(format!("{}.{}.log", $kind, $ordinal))
                .to_string_lossy(),
        );
        let config_filename = format!("{}-{}-{}.toml", $app, $kind, $ordinal);
        let config_path = $paths.workers_available.join(&config_filename);
        let config_content = toml::to_string(&worker_config)?;
        std::fs::write(&config_path, &config_content)?;
        let enabled_path = $paths.workers_enabled.join(&config_filename);
        if enabled_path.exists() {
            std::fs::remove_file(&enabled_path)?;
        }
        std::os::unix::fs::symlink(&config_path, &enabled_path)?;
        echo(
            &format!("-----> Created worker config: {}", config_filename),
            "green",
        );
    }};
}
