use std::sync::{Arc, Barrier};

use super::super::embedded_plugins::{find, EmbeddedPlugin};
use super::*;

fn record_both_at_once(
    plugin_root: &Path,
    first: &'static EmbeddedPlugin,
    second: &'static EmbeddedPlugin,
) {
    let barrier = Arc::new(Barrier::new(2));
    let writers: Vec<_> = [first, second]
        .into_iter()
        .map(|plugin| {
            let root = plugin_root.to_path_buf();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                record_written(plugin, &root)
            })
        })
        .collect();
    for writer in writers {
        let outcome = writer.join().unwrap();
        assert!(outcome.is_ok(), "recording failed: {outcome:?}");
    }
}

#[test]
fn two_installs_at_once_both_end_up_in_the_record() {
    let go = find("go").unwrap();
    let node = find("node").unwrap();
    for round in 0..100 {
        let tmp = tempfile::TempDir::new().unwrap();

        record_both_at_once(tmp.path(), go, node);

        let record = load(tmp.path()).unwrap();
        assert!(
            record.hash_of("go").is_some() && record.hash_of("node").is_some(),
            "round {round}: record holds {:?}",
            record.render()
        );
    }
}
