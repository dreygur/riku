//! Concurrency stress tests guarding two specific fixes against regression:
//!
//! - **App deploy lock** (`deploy::lock`) — concurrent `do_deploy` calls for
//!   the same app must serialize: exactly one wins the `flock`, the rest
//!   fail immediately with `DeployError::DeployInProgress` instead of
//!   racing on git/worker-config/ENV writes.
//! - **Plugin process-group reaping** (`plugins::executor::terminate_process_tree`)
//!   — a hook plugin that backgrounds grandchildren and then hangs past its
//!   timeout must have its *entire* process group killed, not just the
//!   immediate child, so no grandchild survives as an orphan.
//! - **Build-phase resource limits** (`plugins::runtime::build`) — a build
//!   step that tries to balloon its own memory use must be choked by
//!   `ResourceLimits::apply()`'s `RLIMIT_AS`, not run unbounded until it
//!   pressures the host.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;

use riku::config::RikuPaths;
use riku::error::DeployError;
use riku::plugins::hooks::{HookContext, PluginHook};
use riku::plugins::manager::PluginManager;
use riku::plugins::runtime::{RuntimeContext, RuntimePlugin};

/// Tests below mutate process-global env vars (`RIKU_PLUGIN_TIMEOUT`,
/// `RIKU_MAX_MEMORY_MB`) that `ResourceLimits::from_env()` /
/// `plugin_timeout()` read. `cargo test` runs `#[test]` fns concurrently on
/// separate threads of the *same* process by default, so without this lock
/// two such tests running at once could each see the other's env var value.
static ENV_VAR_LOCK: Mutex<()> = Mutex::new(());

/// Build a `RikuPaths` rooted inside `tmp` and create every directory
/// `do_deploy` / `PluginManager` touch.
fn make_paths(tmp: &TempDir) -> RikuPaths {
    let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
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
        fs::create_dir_all(dir).expect("create riku dir");
    }
    paths
}

/// Install a mock runtime plugin that always accepts the app and sleeps
/// briefly during `build` — widening the window during which the deploy
/// lock must be held, so a racing second deploy has time to collide with it
/// instead of the two calls happening to interleave without overlapping.
fn install_slow_mock_plugin(paths: &RikuPaths, name: &str, marker_file: &str, start_cmd: &str) {
    let script = format!(
        r#"#!/usr/bin/env bash
CMD="${{1:-}}"
APP_PATH="${{RIKU_APP_PATH:-$(pwd)}}"
case "$CMD" in
  detect) [ -f "$APP_PATH/{marker}" ] && exit 0; exit 1 ;;
  build)  sleep 0.4; exit 0 ;;
  env)    ;;
  start)  echo "{start}" ;;
  *)      echo "Unknown: $CMD" >&2; exit 1 ;;
esac
"#,
        marker = marker_file,
        start = start_cmd,
    );
    let dest = paths.plugin_root.join(name);
    fs::write(&dest, script).expect("write mock plugin");
    fs::set_permissions(&dest, fs::Permissions::from_mode(0o755)).expect("chmod mock plugin");
}

// ---------------------------------------------------------------------------
// Test 1: concurrent deploys of the same app must serialize on the lock
// ---------------------------------------------------------------------------

/// Five threads call `do_deploy` for the exact same app at the same instant.
/// `sync_app_repo(.., None)` only does a best-effort `git fetch` (ignored on
/// failure) and skips the reset entirely, so this test doesn't need a real
/// git repo — it isolates the lock behavior from git mechanics, which fix
/// #2 (the per-app `flock` in `deploy::lock`) doesn't touch.
///
/// Without the lock, all five threads would race on `git_reset` (skipped
/// here), worker TOML writes, and the ENV file's `NGINX_INTERNAL_PORT`
/// read-modify-write — exactly the corruption fix #2 closes off.
#[test]
fn test_concurrent_deploy_lockout() {
    let tmp = TempDir::new().unwrap();
    let paths = Arc::new(make_paths(&tmp));

    let app = "lockapp";
    install_slow_mock_plugin(&paths, "python", "requirements.txt", "python app.py");

    let app_dir = paths.app_root.join(app);
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("Procfile"), "web: gunicorn app:application\n").unwrap();
    fs::write(app_dir.join("requirements.txt"), "gunicorn==20.0.0\n").unwrap();
    fs::write(app_dir.join("app.py"), "application = None\n").unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();
    fs::write(paths.env_root.join(app).join("ENV"), "PORT=5000\n").unwrap();
    fs::create_dir_all(paths.log_root.join(app)).unwrap();

    const N: usize = 5;
    let barrier = Arc::new(Barrier::new(N));

    let handles: Vec<_> = (0..N)
        .map(|_| {
            let paths = Arc::clone(&paths);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait(); // line every thread up so they hit the lock together
                let deltas: HashMap<String, i64> = HashMap::new();
                riku::deploy::do_deploy(app, &paths, &deltas, None)
            })
        })
        .collect();

    let outcomes: Vec<anyhow::Result<()>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let successes = outcomes.iter().filter(|r| r.is_ok()).count();
    let lock_conflicts = outcomes
        .iter()
        .filter(|r| {
            matches!(
                r.as_ref().err().and_then(|e| e.downcast_ref::<DeployError>()),
                Some(DeployError::DeployInProgress(_))
            )
        })
        .count();

    for (i, outcome) in outcomes.iter().enumerate() {
        if let Err(e) = outcome {
            let is_lock_conflict = matches!(
                e.downcast_ref::<DeployError>(),
                Some(DeployError::DeployInProgress(_))
            );
            assert!(
                is_lock_conflict,
                "thread {} failed for a reason other than the deploy lock: {:?}",
                i, e
            );
        }
    }

    assert_eq!(
        successes, 1,
        "exactly one of {} concurrent deploys of the same app should succeed, got {}",
        N, successes
    );
    assert_eq!(
        lock_conflicts,
        N - 1,
        "every other concurrent deploy should fail with DeployInProgress, got {}",
        lock_conflicts
    );

    // The winning deploy must have actually run to completion, not just
    // acquired the lock and stopped.
    let web_cfg = paths.workers_available.join("lockapp-web-1.toml");
    assert!(
        web_cfg.exists(),
        "the winning deploy should have written the worker config"
    );
}

/// Sequential deploys of the *same* app must each succeed: the lock is
/// released when `do_deploy` returns, not held forever.
#[test]
fn test_deploy_lock_released_after_completion() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let app = "sequentialapp";
    install_slow_mock_plugin(&paths, "python", "requirements.txt", "python app.py");

    let app_dir = paths.app_root.join(app);
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("Procfile"), "web: gunicorn app:application\n").unwrap();
    fs::write(app_dir.join("requirements.txt"), "gunicorn==20.0.0\n").unwrap();
    fs::create_dir_all(paths.env_root.join(app)).unwrap();
    fs::create_dir_all(paths.log_root.join(app)).unwrap();

    let deltas: HashMap<String, i64> = HashMap::new();
    riku::deploy::do_deploy(app, &paths, &deltas, None).expect("first deploy should succeed");
    riku::deploy::do_deploy(app, &paths, &deltas, None)
        .expect("second sequential deploy should succeed once the first released the lock");
}

/// Concurrent deploys of *different* apps must not contend with each other
/// — the lock is keyed per app name, not global.
#[test]
fn test_concurrent_deploys_of_different_apps_do_not_contend() {
    let tmp = TempDir::new().unwrap();
    let paths = Arc::new(make_paths(&tmp));
    install_slow_mock_plugin(&paths, "python", "requirements.txt", "python app.py");

    let apps = ["appx", "appy", "appz"];
    for app in &apps {
        let app_dir = paths.app_root.join(app);
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("Procfile"), "web: gunicorn app:application\n").unwrap();
        fs::write(app_dir.join("requirements.txt"), "gunicorn==20.0.0\n").unwrap();
        fs::create_dir_all(paths.env_root.join(app)).unwrap();
        fs::create_dir_all(paths.log_root.join(app)).unwrap();
    }

    let barrier = Arc::new(Barrier::new(apps.len()));
    let handles: Vec<_> = apps
        .iter()
        .map(|&app| {
            let paths = Arc::clone(&paths);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let deltas: HashMap<String, i64> = HashMap::new();
                riku::deploy::do_deploy(app, &paths, &deltas, None)
            })
        })
        .collect();

    for (app, handle) in apps.iter().zip(handles) {
        let result = handle.join().unwrap();
        assert!(
            result.is_ok(),
            "deploy of '{}' should not be blocked by other apps' locks: {:?}",
            app,
            result.err()
        );
    }
}

// ---------------------------------------------------------------------------
// Test 2: a timed-out hook plugin must have its whole process group reaped
// ---------------------------------------------------------------------------

/// `riku-pre-deploy` backgrounds two long-sleeping grandchildren, records
/// their PIDs to a file, then itself hangs past the configured timeout.
/// Before fix #3, `wait_with_timeout` only `Child::kill()`ed the immediate
/// hook process — the backgrounded `sleep`s, left in the same process group
/// (`process_group(0)` makes the hook process the group leader; bash's `&`
/// jobs without job control inherit that same pgid), survived as orphans.
/// After the fix, `terminate_process_tree` detects the hook is its own
/// group leader and `killpg`s the whole group.
#[test]
fn test_plugin_process_group_reaping() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let pidfile = tmp.path().join("bg_pids.txt");
    let script = format!(
        r#"#!/usr/bin/env bash
( sleep 100 & echo $! >> "{pidfile}" )
( sleep 100 & echo $! >> "{pidfile}" )
sleep 100
"#,
        pidfile = pidfile.display(),
    );
    let plugin_path = paths.plugin_root.join("riku-pre-deploy");
    fs::write(&plugin_path, script).expect("write hook script");
    fs::set_permissions(&plugin_path, fs::Permissions::from_mode(0o755)).expect("chmod hook");

    let app_path = tmp.path().join("app");
    let env_path = tmp.path().join("env");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(&env_path).unwrap();
    let app_env = HashMap::new();

    let ctx = HookContext {
        app: "stressapp",
        hook: &PluginHook::PreDeploy,
        app_path: &app_path,
        env_path: &env_path,
        riku_root: &paths.riku_root,
        runtime: None,
        app_env: &app_env,
    };

    let manager = PluginManager::new(&paths);

    // Holds ENV_VAR_LOCK for the entire env-var-dependent section so this
    // doesn't race test_build_phase_memory_limit_chokes_runaway_allocation,
    // which mutates its own (different) env vars on another thread.
    let (result, elapsed) = {
        let _guard = ENV_VAR_LOCK.lock().unwrap();
        // Tight timeout: well under the children's `sleep 100`, so the
        // timeout path — not a normal exit — is what tears everything down.
        std::env::set_var("RIKU_PLUGIN_TIMEOUT", "1");

        let start = Instant::now();
        let result = manager.run_hook(&ctx);
        let elapsed = start.elapsed();

        std::env::remove_var("RIKU_PLUGIN_TIMEOUT");
        (result, elapsed)
    };

    // PreDeploy is abort-on-timeout, so a timed-out hook must surface as Err.
    assert!(
        result.is_err(),
        "a PreDeploy hook that times out must return Err, got {:?}",
        result
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "run_hook should return shortly after the ~1s timeout, not wait for sleep 100 (took {:?})",
        elapsed
    );

    // Give the kernel a moment to finish tearing down the killed processes.
    std::thread::sleep(Duration::from_millis(300));

    let pid_text = fs::read_to_string(&pidfile).unwrap_or_default();
    let pids: Vec<i32> = pid_text
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect();
    assert!(
        !pids.is_empty(),
        "the hook script should have recorded at least one backgrounded grandchild PID"
    );

    for pid in pids {
        let proc_path = format!("/proc/{}", pid);
        assert!(
            !Path::new(&proc_path).exists(),
            "backgrounded grandchild PID {} survived the hook timeout — killpg did not reap the process group",
            pid
        );
    }
}

// ---------------------------------------------------------------------------
// Test 3: a runaway build step must be choked by RLIMIT_AS, not run unbounded
// ---------------------------------------------------------------------------

/// The mock runtime plugin's `build` subcommand tries to balloon its own
/// memory use. Growth is in fixed 10MB chunks up to a hard ceiling of 300MB
/// — bounded by construction, so even if `ResourceLimits::apply()` were
/// somehow *not* wired into `plugins::runtime::build()` (the regression
/// this test guards against), the worst case is one bash process briefly
/// holding ~300MB, never an unbounded climb that could pressure the host
/// the way an unconstrained fork-bomb or memory grab would.
fn write_memory_hog_plugin(paths: &RikuPaths, name: &str) -> PathBuf {
    let script = r#"#!/usr/bin/env bash
case "${1:-}" in
  build)
    chunk=$(head -c 10000000 /dev/zero | tr '\0' 'x')
    big=""
    for i in $(seq 1 30); do
      big="$big$chunk"
    done
    echo "allocated ${#big} bytes" >&2
    ;;
  *) exit 0 ;;
esac
"#;
    let dest = paths.plugin_root.join(name);
    fs::write(&dest, script).expect("write memory-hog plugin");
    fs::set_permissions(&dest, fs::Permissions::from_mode(0o755)).expect("chmod memory-hog plugin");
    dest
}

/// `plugins::runtime::build()` applies `ResourceLimits::apply()` (RLIMIT_AS
/// included) via `pre_exec` before exec'ing the plugin's `build`
/// subcommand (fix #6 — this used to run completely unbounded). With
/// `RIKU_MAX_MEMORY_MB=64`, the plugin's attempt to grow past 64MB must
/// fail fast: either bash's own allocator reports "cannot allocate memory"
/// and exits non-zero, or the kernel kills it for an `mmap`/`brk` that
/// would exceed RLIMIT_AS — either way `build()` must return `Err`, and it
/// must do so well before the *unrelated* plugin-timeout backstop would
/// have fired, proving the resource limit — not the timeout — is what
/// stopped it.
#[test]
fn test_build_phase_memory_limit_chokes_runaway_allocation() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let plugin_path = write_memory_hog_plugin(&paths, "memhog");
    let plugin = RuntimePlugin {
        name: "memhog".to_string(),
        path: plugin_path,
    };

    let app_path = tmp.path().join("app");
    let env_path = tmp.path().join("env");
    fs::create_dir_all(&app_path).unwrap();
    fs::create_dir_all(&env_path).unwrap();
    let app_env = HashMap::new();

    let ctx = RuntimeContext {
        app: "memhogapp",
        app_path: &app_path,
        env_path: &env_path,
        riku_root: &paths.riku_root,
        app_env: &app_env,
    };

    let (result, elapsed) = {
        let _guard = ENV_VAR_LOCK.lock().unwrap();
        // Small cap so the limit trips almost immediately (well within the
        // 30 * 10MB = 300MB ceiling the script itself is bounded to), and a
        // generous timeout backstop that should never actually be needed —
        // its only job here is to prove it *wasn't* what stopped the build.
        std::env::set_var("RIKU_MAX_MEMORY_MB", "64");
        std::env::set_var("RIKU_PLUGIN_TIMEOUT", "15");

        let start = Instant::now();
        let result = riku::plugins::runtime::build(&plugin, &ctx);
        let elapsed = start.elapsed();

        std::env::remove_var("RIKU_MAX_MEMORY_MB");
        std::env::remove_var("RIKU_PLUGIN_TIMEOUT");
        (result, elapsed)
    };

    let err = result.expect_err("a build step that exceeds RIKU_MAX_MEMORY_MB must fail, got Ok");
    assert!(
        elapsed < Duration::from_secs(5),
        "build() took {:?} — should fail almost immediately once RLIMIT_AS is \
         exceeded; taking close to the 15s timeout would mean the timeout, \
         not the memory limit, is what actually stopped it",
        elapsed
    );

    // The failure must surface as the structured resource-exhaustion
    // diagnostic, not a bare "exited with code N" — i.e. classify_resource_exit
    // must have actually matched the allocator's "xrealloc: cannot allocate"
    // message on the captured stderr tail, and DeployError::resource_exhausted
    // must have built the labeled diagnostic block from it.
    let message = err.to_string();
    assert!(
        matches!(
            err.downcast_ref::<riku::error::DeployError>(),
            Some(riku::error::DeployError::ResourceExhausted(_))
        ),
        "expected DeployError::ResourceExhausted, got: {}",
        message
    );
    for field in ["stage", "command", "cause", "impact", "remedy"] {
        assert!(
            message.contains(field),
            "diagnostic missing '{}' field:\n{}",
            field,
            message
        );
    }
    assert!(
        message.contains("RIKU_MAX_MEMORY_MB"),
        "diagnostic should name the specific limit knob to adjust:\n{}",
        message
    );
    assert!(
        message.to_lowercase().contains("xrealloc")
            || message.to_lowercase().contains("cannot allocate"),
        "diagnostic should surface what the allocator actually reported:\n{}",
        message
    );
}
