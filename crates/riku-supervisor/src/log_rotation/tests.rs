use super::*;
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_log_rotator_creation() {
    let rotator = LogRotator::with_defaults();
    assert_eq!(rotator.config.max_size, 10 * 1024 * 1024);
    assert_eq!(rotator.config.retention_count, 5);
}

#[test]
fn test_needs_rotation() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("test.log");

    // Create small log file
    let mut file = File::create(&log_path).unwrap();
    writeln!(file, "Small log entry").unwrap();

    let rotator = LogRotator::with_defaults();
    assert!(!rotator.needs_rotation(&log_path).unwrap());
}

#[test]
fn test_rotate_log() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("test.log");

    // Create log file with content
    let mut file = File::create(&log_path).unwrap();
    writeln!(file, "Log content").unwrap();

    let rotator = LogRotator::new(LogRotationConfig {
        max_size: 0, // Force rotation
        retention_count: 3,
    });

    rotator.rotate(&log_path).unwrap();

    // Original file should exist and be empty
    assert!(log_path.exists());
    assert_eq!(std::fs::read_to_string(&log_path).unwrap(), "");

    // Rotated file should exist with content
    let rotated_path = temp_dir.path().join("test.log.1");
    assert!(rotated_path.exists());
    assert!(std::fs::read_to_string(&rotated_path)
        .unwrap()
        .contains("Log content"));
}

#[test]
fn test_log_size() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("test.log");

    let mut file = File::create(&log_path).unwrap();
    writeln!(file, "Test content").unwrap();

    let size = get_log_size(&log_path).unwrap();
    assert!(size > 0);
}
