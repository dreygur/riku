use crate::testkit::{put, rel};
use crate::{receive, scan, send, SendReport};
use std::fs;
use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;

fn sync_over_sockets(src: &Path, live: &Path) -> anyhow::Result<SendReport> {
    let (client_out, server_in) = UnixStream::pair().unwrap();
    let (server_out, client_in) = UnixStream::pair().unwrap();
    let live = live.to_path_buf();
    let server = thread::spawn(move || receive(&live, BufReader::new(server_in), server_out));
    let report = send(src, BufReader::new(client_in), client_out);
    let served = server.join().unwrap();
    assert!(served.is_ok(), "server side failed: {served:?}");
    report
}

#[test]
fn second_sync_moves_only_the_changed_file() {
    let src = tempfile::tempdir().unwrap();
    put(src.path(), "app.py", "v1");
    put(src.path(), "old.txt", "x");
    put(src.path(), "lib/util.py", "u");
    let parent = tempfile::tempdir().unwrap();
    let live = parent.path().join("live");

    let first = sync_over_sockets(src.path(), &live).unwrap();
    assert_eq!(
        first.sent,
        vec![rel("app.py"), rel("lib/util.py"), rel("old.txt")],
        "first sync must send every file"
    );
    assert!(first.bytes > 0, "no bytes counted for the first bundle");

    put(src.path(), "app.py", "v2");
    fs::remove_file(src.path().join("old.txt")).unwrap();
    let second = sync_over_sockets(src.path(), &live).unwrap();
    assert_eq!(second.sent, vec![rel("app.py")]);
    assert_eq!(second.deleted, vec![rel("old.txt")]);
    assert_eq!(scan(&live).unwrap(), scan(src.path()).unwrap());
}
