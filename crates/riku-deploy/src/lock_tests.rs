use super::*;
use tempfile::TempDir;

fn make_paths(tmp: &TempDir) -> RikuPaths {
    RikuPaths::for_tests(tmp.path())
}

fn is_deploy_in_progress(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<DeployError>(),
        Some(DeployError::DeployInProgress(_))
    )
}

#[test]
fn test_acquire_succeeds_when_unlocked() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    assert!(acquire("myapp", &paths).is_ok());
}

#[test]
fn test_acquire_fails_when_already_locked() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let _first = acquire("myapp", &paths).expect("first lock should succeed");
    let second = acquire("myapp", &paths);

    assert!(second.is_err(), "second concurrent lock should fail");
    let err = second.unwrap_err();
    assert!(
        is_deploy_in_progress(&err),
        "error should be DeployError::DeployInProgress, got: {:?}",
        err
    );
}

#[test]
fn test_acquire_succeeds_again_after_lock_dropped() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    {
        let _first = acquire("myapp", &paths).expect("first lock should succeed");
    } // dropped here, releasing the flock

    // Poll rather than asserting instantly: a concurrent test in this
    // process can `fork()` a child that transiently inherits the lock fd
    // (see `eventually` / `test_is_locked_false_after_release`).
    assert!(
        eventually(|| acquire("myapp", &paths).is_ok()),
        "lock should be acquirable again once released"
    );
}

#[test]
fn test_acquire_different_apps_do_not_conflict() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let _a = acquire("app-a", &paths).expect("app-a lock should succeed");
    let _b = acquire("app-b", &paths).expect("app-b lock should succeed independently");
}

#[test]
fn acquire_waits_out_a_briefly_held_lock() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let lock = acquire("myapp", &paths).expect("first lock should succeed");
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        drop(lock);
    });

    assert!(
        acquire("myapp", &paths).is_ok(),
        "acquire should wait out the briefly held lock and succeed"
    );
    handle.join().unwrap();
}

#[test]
fn a_deploy_waits_for_a_restart_that_outlasts_the_contention_grace() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let held_for = CONTENTION_GRACE + Duration::from_millis(500);

    let restart = acquire_for_restart("myapp", &paths).expect("restart lock should succeed");
    let handle = std::thread::spawn(move || {
        std::thread::sleep(held_for);
        drop(restart);
    });

    let start = Instant::now();
    let deploy = acquire_for_deploy("myapp", &paths);
    let waited = start.elapsed();
    handle.join().unwrap();

    assert!(
        deploy.is_ok(),
        "a deploy must wait for the restart, got: {:?}",
        deploy.err()
    );
    assert!(
        waited >= CONTENTION_GRACE,
        "the deploy should have waited past the grace, waited {waited:?}"
    );
}

#[test]
fn a_deploy_is_still_refused_while_another_deploy_holds_the_lock() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let _first = acquire_for_deploy("myapp", &paths).expect("first deploy should lock");
    let start = Instant::now();
    let second = acquire_for_deploy("myapp", &paths);

    match second {
        Ok(_) => panic!("a second deploy must be refused"),
        Err(e) => assert!(is_deploy_in_progress(&e), "got: {e:?}"),
    }
    assert!(
        start.elapsed() < RESTART_WAIT,
        "only a restart holder may stretch the wait, waited {:?}",
        start.elapsed()
    );
}

// --- is_locked ---

#[test]
fn test_is_locked_false_when_never_acquired() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    assert!(!is_locked("myapp", &paths));
}

#[test]
fn test_is_locked_true_while_held() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let _held = acquire("myapp", &paths).unwrap();
    assert!(is_locked("myapp", &paths));
}

#[test]
fn test_is_locked_false_after_release() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    {
        let _held = acquire("myapp", &paths).unwrap();
        assert!(is_locked("myapp", &paths));
    }
    // The lock is released as soon as the holder fd is dropped above.
    // However, when other tests in this process `fork()` a child while
    // this fd is briefly open, the child transiently inherits the lock's
    // open file description (fork copies all fds; O_CLOEXEC only closes it
    // at the child's `exec`, not at `fork`). For that window the lock can
    // still be observed as held through the child. It settles to free once
    // the child execs/exits, so we poll rather than asserting an instant
    // flip.
    assert!(
        eventually(|| !is_locked("myapp", &paths)),
        "lock should become free after the holder is dropped"
    );
}

/// Poll a lock predicate until it holds, up to ~10s (returns false on
/// timeout). The uncontended case returns on the first iteration; the long
/// ceiling only matters when a concurrent test's child is slow to `exec`
/// (and thus slow to drop the inherited lock fd) under heavy load. See
/// `test_is_locked_false_after_release` for the full rationale.
fn eventually(mut pred: impl FnMut() -> bool) -> bool {
    for _ in 0..200 {
        if pred() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    false
}

#[test]
fn test_is_locked_probe_does_not_itself_hold_the_lock() {
    // Calling is_locked() on a free lock must not leave it held for a
    // subsequent real acquire(): the probe releases what it took.
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    assert!(!is_locked("myapp", &paths));
    assert!(
        acquire("myapp", &paths).is_ok(),
        "a probe must never leave the lock held behind it"
    );
}

#[test]
fn test_lock_path_includes_app_name() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let path = lock_path_for("myapp", &paths);
    assert!(path.to_string_lossy().contains("myapp"));
}

#[test]
fn test_lock_file_persists_on_disk() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let _lock = acquire("myapp", &paths).unwrap();
    assert!(Path::new(&lock_path_for("myapp", &paths)).exists());
}
