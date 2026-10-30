//! Reading one newline terminated line from a stream the peer controls.
//!
//! Security model: the peer picks the length. The line is refused as soon as
//! it would pass the cap, before those bytes are copied, and the buffer never
//! grows past the cap, so a peer that never sends a newline costs at most `cap`
//! bytes of memory.

use std::io::{self, BufRead};

/// Reads up to and including the next `\n` and returns the bytes before it.
///
/// A line longer than `cap` bytes, or a stream that ends before the newline, is
/// an error naming `what`. Bytes after the newline stay unread.
pub(crate) fn read_line(
    reader: &mut impl BufRead,
    cap: usize,
    what: &str,
) -> anyhow::Result<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        let chunk = match reader.fill_buf() {
            Ok(chunk) => chunk,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(anyhow::anyhow!("cannot read the {what}: {e}")),
        };
        if chunk.is_empty() {
            return Err(anyhow::anyhow!(
                "stream ended before the {what} was complete"
            ));
        }
        let newline = chunk.iter().position(|byte| *byte == b'\n');
        let take = newline.unwrap_or(chunk.len());
        if line.len() + take > cap {
            return Err(anyhow::anyhow!("{what} is longer than {cap} bytes"));
        }
        if let Err(e) = reserve_within(&mut line, take, cap) {
            return Err(anyhow::anyhow!("cannot buffer the {what}: {e}"));
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
