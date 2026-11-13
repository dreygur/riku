use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::display::OutputTail;

fn deploys_in(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.split_once("| ").map(|(_, rest)| rest.to_string()))
        .collect()
}

/// Build and release output reaches the terminal only through the tail,
/// so the tail is what puts it in the deploy log.
#[test]
fn every_streamed_line_lands_in_the_deploy_log() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("logs/shop/deploy.log");
    let log = DeployLogger::new(&path).unwrap();
    let tail = OutputTail::default().logged_to(log.sink());

    for n in 0..40 {
        tail.push(&format!("compiling unit {n}"));
    }

    let text = fs::read_to_string(&path).unwrap();
    let streamed = deploys_in(&text);
    assert_eq!(streamed.len(), 40, "log:\n{text}");
    assert_eq!(streamed[0], "compiling unit 0");
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "build output may hold secrets");
}

#[test]
fn opening_the_log_keeps_only_the_most_recent_deploys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deploy.log");
    let limits = LogLimits {
        deploys: 3,
        bytes: 1 << 20,
    };

    for n in 0..5 {
        let log = DeployLogger::open(&path, limits).unwrap();
        log.sink().write_streamed(&format!("deploy {n}"));
    }

    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(
        deploys_in(&text),
        ["deploy 2", "deploy 3", "deploy 4"],
        "log:\n{text}"
    );
}

/// The byte cap holds at the next deploy's start, and one deploy's output
/// cannot grow the file past it while it runs.
#[test]
fn the_log_stays_under_its_byte_cap() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deploy.log");
    let limits = LogLimits {
        deploys: 10,
        bytes: 2_000,
    };

    for n in 0..3 {
        let log = DeployLogger::open(&path, limits).unwrap();
        for line in 0..100 {
            log.sink()
                .write_streamed(&format!("deploy {n} line {line}"));
        }
    }

    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.len() <= 2 * limits.bytes,
        "log grew to {} bytes",
        text.len()
    );
    assert!(text.contains(TRUNCATED), "no truncation notice:\n{text}");
    let first = text.lines().next().unwrap_or("");
    assert!(
        crate::log_stamp::split_stamp(first).is_some_and(|(_, rest)| rest == DEPLOY_MARK),
        "a trimmed log must start at a deploy, got {first:?}"
    );
}
