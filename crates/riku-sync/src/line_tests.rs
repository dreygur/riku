use super::*;
use std::io::{BufReader, Cursor};

#[test]
fn returns_one_line_and_leaves_the_rest_unread() {
    let mut stream = Cursor::new(b"first\nsecond\n".to_vec());
    let line = read_line(&mut stream, 64, "line").unwrap();
    assert_eq!(line, b"first", "the newline must not be part of the line");
    let rest = read_line(&mut stream, 64, "line").unwrap();
    assert_eq!(
        rest, b"second",
        "bytes after the first newline were consumed"
    );
}

#[test]
fn overlong_line_is_refused_with_memory_held_to_the_cap() {
    let cap = 100;
    let flood = vec![b'a'; cap * 50];
    let err = read_line(
        &mut BufReader::with_capacity(8, flood.as_slice()),
        cap,
        "manifest line",
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("manifest line") && err.contains("100"),
        "error must name the line and the cap, got: {err}"
    );
    let mut exact = Cursor::new([vec![b'a'; cap], b"\n".to_vec()].concat());
    let line = read_line(&mut exact, cap, "line").unwrap();
    assert_eq!(line.len(), cap, "a line of exactly the cap is allowed");
    assert!(
        line.capacity() <= cap,
        "buffer grew to {} for a cap of {cap}",
        line.capacity()
    );
}

#[test]
fn stream_ending_inside_a_line_is_an_error() {
    for cut in [&b""[..], &b"no newline"[..]] {
        let err = read_line(&mut Cursor::new(cut.to_vec()), 64, "plan reply");
        assert!(
            err.is_err(),
            "{:?} must not count as a complete line",
            String::from_utf8_lossy(cut)
        );
    }
}
