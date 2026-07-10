use super::super::test_support::minimal_config;
use super::{reopen_if_rotated, run_log_capture_thread};
use crate::process::ProcessManager;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use tempfile::TempDir;

// ── reopen_if_rotated ────────────────────────────────────────────────────

#[test]
fn test_reopen_if_rotated_noop_when_unchanged() {
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("app.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap();
    let original_ino = file.metadata().unwrap().ino();

    reopen_if_rotated(&log_path, &mut file, "stdout");

    assert_eq!(
        file.metadata().unwrap().ino(),
        original_ino,
        "an untouched log file must not be reopened"
    );
}

#[test]
fn test_reopen_if_rotated_detects_rename_and_recreate() {
    // The conventional (non-copytruncate) logrotate strategy: rename
    // the live file away, then something (logrotate's `create`
    // directive, or just the next writer) creates a fresh file at the
    // original path.
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("app.log");
    let rotated_path = tmp.path().join("app.log.1");

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap();
    writeln!(file, "before rotation").unwrap();
    let old_ino = file.metadata().unwrap().ino();

    fs::rename(&log_path, &rotated_path).unwrap();
    fs::write(&log_path, "").unwrap(); // external tool recreates the path

    reopen_if_rotated(&log_path, &mut file, "stdout");

    let new_ino = file.metadata().unwrap().ino();
    assert_ne!(
        old_ino, new_ino,
        "file handle must point at the new inode after rotation"
    );
    assert_eq!(
        new_ino,
        fs::metadata(&log_path).unwrap().ino(),
        "reopened handle must match the inode currently at log_path"
    );

    // Confirm writes through the reopened handle land in the NEW file,
    // not the renamed-away copy.
    writeln!(file, "after rotation").unwrap();
    file.flush().unwrap();
    let new_content = fs::read_to_string(&log_path).unwrap();
    assert!(new_content.contains("after rotation"));
    assert!(!new_content.contains("before rotation"));

    let rotated_content = fs::read_to_string(&rotated_path).unwrap();
    assert!(rotated_content.contains("before rotation"));
}

#[test]
fn test_reopen_if_rotated_keeps_old_handle_when_path_missing() {
    // Deleted but not yet recreated (e.g. mid-rotation race, or an
    // operator running `rm` directly): must not panic, and must keep
    // the existing — still valid, just unlinked — handle rather than
    // erroring.
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("app.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap();
    let original_ino = file.metadata().unwrap().ino();

    fs::remove_file(&log_path).unwrap();

    reopen_if_rotated(&log_path, &mut file, "stdout");

    assert_eq!(
        file.metadata().unwrap().ino(),
        original_ino,
        "handle must be unchanged while the path is missing"
    );
    // The unlinked fd is still fully writable.
    writeln!(file, "still alive").unwrap();
    file.flush().unwrap();
}

// ── run_log_capture_thread ───────────────────────────────────────────────

/// A `Read` impl that yields one chunk, then blocks (via a channel
/// recv) until the test signals it to yield EOF — long enough to let
/// the test perform an external rotation while the capture loop is
/// genuinely mid-stream, not finished.
struct PausableReader {
    chunk: Option<Vec<u8>>,
    resume: std::sync::mpsc::Receiver<()>,
}

impl Read for PausableReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(chunk) = self.chunk.take() {
            let n = chunk.len().min(buf.len());
            buf[..n].copy_from_slice(&chunk[..n]);
            return Ok(n);
        }
        // Block until the test is done manipulating the filesystem,
        // then report EOF so the capture loop exits cleanly.
        let _ = self.resume.recv();
        Ok(0)
    }
}

#[test]
fn test_run_log_capture_thread_survives_external_rotation_mid_stream() {
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("app.log");
    let rotated_path = tmp.path().join("app.log.1");

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap();

    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let reader = PausableReader {
        chunk: Some(b"line one\n".to_vec()),
        resume: resume_rx,
    };

    let path_for_thread = log_path.clone();
    let handle = std::thread::spawn(move || {
        run_log_capture_thread(reader, &path_for_thread, file, "stdout");
    });

    // Give the thread time to write "line one" and reach the blocking
    // read for the next chunk.
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Rotate externally while the thread is alive and holding the old fd.
    fs::rename(&log_path, &rotated_path).unwrap();
    fs::write(&log_path, "").unwrap();

    // Let the reader hit EOF so the thread exits — run_log_capture_thread
    // checks for rotation on a wall-clock interval, but the test only
    // needs to prove the *mechanism* (reopen_if_rotated, covered above);
    // here we're proving the surrounding thread doesn't panic or hang
    // across a rotation event while it's actively running.
    let _ = resume_tx.send(());
    handle.join().expect("capture thread must not panic");

    // Whichever file "line one" landed in, it must be exactly one of
    // the two — never silently lost, never duplicated, never corrupted.
    let original_content = fs::read_to_string(&log_path).unwrap_or_default();
    let rotated_content = fs::read_to_string(&rotated_path).unwrap_or_default();
    assert_eq!(
        original_content.matches("line one").count() + rotated_content.matches("line one").count(),
        1,
        "the line written before rotation must appear exactly once across both files"
    );
}

#[test]
fn test_open_log_files_creates_file_and_dirs() {
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("subdir").join("app.log");
    let handles = ProcessManager::open_log_files(log_path.to_str().unwrap())
        .expect("open_log_files should succeed");
    assert!(handles.is_some(), "should return file handles");
    assert!(log_path.exists(), "log file should be created on disk");
}

#[test]
fn test_spawn_process_echo_succeeds() {
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("test.log");

    let config = minimal_config(
        "echo hello",
        tmp.path().to_str().unwrap(),
        log_path.to_str().unwrap(),
    );

    let mut pm = ProcessManager::new().expect("ProcessManager::new should succeed");
    pm.spawn_process(&config)
        .expect("spawning 'echo hello' should succeed");

    // Allow log-capture threads to drain before asserting count.
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(
        pm.get_process_count(),
        1,
        "one process should be registered"
    );
}

#[test]
fn test_spawn_duplicate_process_id_replaces_old() {
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("test.log");

    let config = minimal_config(
        "sleep 60",
        tmp.path().to_str().unwrap(),
        log_path.to_str().unwrap(),
    );

    let mut pm = ProcessManager::new().expect("ProcessManager::new should succeed");
    pm.spawn_process(&config)
        .expect("first spawn should succeed");
    assert_eq!(pm.get_process_count(), 1);

    // Spawning again with the same app/kind/ordinal replaces the old entry.
    pm.spawn_process(&config)
        .expect("second spawn should succeed");
    assert_eq!(
        pm.get_process_count(),
        1,
        "duplicate should replace, not add"
    );
}
