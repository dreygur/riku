use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use riku_container::engine::{ContainerId, ContainerInfo, ContainerState};
use riku_container::types::{CopyIndex, ServiceName};

use super::log_restart;
use crate::containers::tally::RestartTally;

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn restart_is_logged_with_app_service_copy_exit_code_and_count() {
    let info = ContainerInfo {
        id: ContainerId::try_from(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .unwrap(),
        name: "dockerapp.app.1".to_string(),
        app: Some("dockerapp".to_string()),
        service: Some(ServiceName::try_from("app").unwrap()),
        copy: Some(CopyIndex::new(1).unwrap()),
        image: "dockerapp:1".to_string(),
        image_id: "sha256:test".to_string(),
        host_port: None,
        restart: Some("unless-stopped".to_string()),
        spec: None,
        state: ContainerState::Exited { code: 137 },
    };
    let mut tally = RestartTally::new();
    tally.observe(&info.name, &info.id);
    tally.record_restart(&info.name, Instant::now());
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_ansi(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || log_restart(&info, 137, &tally));

    let line = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    for field in [
        "INFO",
        "app=\"dockerapp\"",
        "service=app",
        "copy=1",
        "exit_code=137",
        "restart=2",
    ] {
        assert!(line.contains(field), "missing {field} in: {line}");
    }
}
