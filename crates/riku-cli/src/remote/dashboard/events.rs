//! Reader threads that turn the remote session's stdout and stderr into one
//! ordered stream of events for the session loop.
//!
//! stderr is read in raw chunks, not lines, because sudo's prompt has no
//! newline. The threads end when their pipe closes, which with ssh
//! connection sharing can be the master closing it, not the client.

use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::process::{ChildStderr, ChildStdout};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

const CHUNK: usize = 4096;

pub(super) enum Event {
    Stderr(Vec<u8>),
    StderrClosed,
    /// One stdout line, newline included when there was one.
    StdoutLine(String),
    StdoutClosed,
}

pub(super) fn read_stdout(stdout: ChildStdout, events: Sender<Event>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&line).into_owned();
                    if events.send(Event::StdoutLine(text)).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = events.send(Event::StdoutClosed);
    })
}

pub(super) fn read_stderr(mut stderr: ChildStderr, events: Sender<Event>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut chunk = [0u8; CHUNK];
        loop {
            match stderr.read(&mut chunk) {
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if events.send(Event::Stderr(chunk[..n].to_vec())).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = events.send(Event::StderrClosed);
    })
}
