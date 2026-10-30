//! Reading one newline terminated line from a stream the peer controls.
//!
//! Security model: the peer picks the length. The line is refused as soon as
//! it would pass the cap, before those bytes are copied, and the buffer never
//! grows past the cap, so a peer that never sends a newline costs at most `cap`
//! bytes of memory.

use std::collections::TryReserveError;
use std::fmt;
use std::io::{self, BufRead};

/// Why a line could not be read. `Display` is for the local log and may carry
/// an OS error; [`LineError::fixed_text`] is what a peer may be told.
#[derive(Debug)]
pub(crate) enum LineError {
    TooLong {
        what: String,
        cap: usize,
    },
    Ended {
        what: String,
    },
    Io {
        what: String,
        source: io::Error,
    },
    Buffer {
        what: String,
        source: TryReserveError,
    },
}

impl LineError {
    /// A text that contains nothing from the OS or the peer.
    pub(crate) fn fixed_text(&self) -> String {
        match self {
            LineError::TooLong { .. } | LineError::Ended { .. } => self.to_string(),
            LineError::Io { what, .. } | LineError::Buffer { what, .. } => {
                format!("the {what} could not be read")
            }
        }
    }
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineError::TooLong { what, cap } => write!(f, "{what} is longer than {cap} bytes"),
            LineError::Ended { what } => {
                write!(f, "stream ended before the {what} was complete")
            }
            LineError::Io { what, source } => write!(f, "cannot read the {what}: {source}"),
            LineError::Buffer { what, source } => write!(f, "cannot buffer the {what}: {source}"),
        }
    }
}

impl std::error::Error for LineError {}

/// Reads up to and including the next `\n` and returns the bytes before it.
///
/// A line longer than `cap` bytes, or a stream that ends before the newline, is
/// an error naming `what`. Bytes after the newline stay unread.
pub(crate) fn read_line(
    reader: &mut impl BufRead,
    cap: usize,
    what: &str,
) -> Result<Vec<u8>, LineError> {
    let mut line = Vec::new();
    loop {
        let chunk = match reader.fill_buf() {
            Ok(chunk) => chunk,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(source) => {
                return Err(LineError::Io {
                    what: what.to_string(),
                    source,
                })
            }
        };
        if chunk.is_empty() {
            return Err(LineError::Ended {
                what: what.to_string(),
            });
        }
        let newline = chunk.iter().position(|byte| *byte == b'\n');
        let take = newline.unwrap_or(chunk.len());
        if line.len() + take > cap {
            return Err(LineError::TooLong {
                what: what.to_string(),
                cap,
            });
        }
        if let Err(source) = reserve_within(&mut line, take, cap) {
            return Err(LineError::Buffer {
                what: what.to_string(),
                source,
            });
        }
        line.extend_from_slice(&chunk[..take]);
        reader.consume(take + usize::from(newline.is_some()));
        if newline.is_some() {
            return Ok(line);
        }
    }
}

/// Makes room for `extra` more bytes, doubling but never beyond `cap` in total.
fn reserve_within(
    line: &mut Vec<u8>,
    extra: usize,
    cap: usize,
) -> Result<(), std::collections::TryReserveError> {
    let needed = line.len() + extra;
    if needed <= line.capacity() {
        return Ok(());
    }
    let target = needed.max(line.capacity().saturating_mul(2)).min(cap);
    line.try_reserve_exact(target - line.len())
}

#[cfg(test)]
#[path = "line_tests.rs"]
mod tests;
