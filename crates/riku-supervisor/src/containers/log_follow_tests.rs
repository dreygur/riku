use std::fs;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

use riku_container::engine::{ContainerId, ContainerInfo, ContainerState};
use riku_container::types::{CopyIndex, ServiceName};

use super::{LogFollowers, Tail};
use crate::containers::mock_engine::{logged, mock_engine};

#[test]
fn follower_appends_engine_output_to_copy_log() {
    let dir = TempDir::new().unwrap();
    let id_str = "1111111111111111111111111111111111111111111111111111111111111111";
    let id = ContainerId::try_from(id_str).unwrap();

    let behaviour = "case \"$1\" in logs) echo 'line one';; esac";
    let (engine, argv_log) = mock_engine(dir.path(), behaviour);

    let log_root = dir.path().join("logs");
    let mut followers = LogFollowers::new(log_root.clone());

    let info = ContainerInfo {
        id: id.clone(),
        name: "myapp-web-1".to_string(),
        app: Some("myapp".to_string()),
        service: Some(ServiceName::try_from("web").unwrap()),
        copy: Some(CopyIndex::new(1).unwrap()),
        image: "myapp:v1".to_string(),
        image_id: "sha256:test".to_string(),
        host_port: None,
        restart: None,
        state: ContainerState::Running,
    };

    followers
        .follow(&engine, &info, Tail::FromNow)
        .expect("follow must succeed");

    let log_file = log_root.join("myapp").join("web.1.log");
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut found = false;

    while Instant::now() < deadline {
        if log_file.exists() {
            let content = fs::read_to_string(&log_file).unwrap_or_default();
            if content.contains("line one") {
                found = true;
                break;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }

    assert!(found, "copy log file must contain 'line one' within 5 s");

    let argv_lines = logged(&argv_log);
    let expected_argv = format!("logs -f --tail 0 {}", id.as_str());
    assert!(
        argv_lines.iter().any(|line| line == &expected_argv),
        "argv.log must record expected command: {expected_argv:?}, found: {argv_lines:?}"
    );
}
