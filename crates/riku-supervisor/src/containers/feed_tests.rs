use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

use riku_container::engine::ContainerId;

use super::EventStream;
use crate::containers::mock_engine::mock_engine;

#[test]
fn event_stream_delivers_ids_then_closes() {
    let dir = TempDir::new().unwrap();
    let known_id_str = "1111111111111111111111111111111111111111111111111111111111111111";
    let expected_id = ContainerId::try_from(known_id_str).unwrap();

    let behaviour = format!(
        "case \"$1\" in events) echo '{{\"Actor\":{{\"ID\":\"{known_id_str}\"}},\"Action\":\"die\"}}';; esac"
    );
    let (engine, _log) = mock_engine(dir.path(), &behaviour);

    let mut stream = EventStream::open(&engine).expect("failed to open event stream");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut collected = Vec::new();
    let mut is_closed = false;

    while Instant::now() < deadline {
        let drained = stream.drain();
        collected.extend(drained.ids);
        if drained.closed {
            is_closed = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    assert!(is_closed, "event stream must report closed within 5 s");
    assert_eq!(
        collected,
        vec![expected_id],
        "event stream must deliver the container id from the die event"
    );
}
