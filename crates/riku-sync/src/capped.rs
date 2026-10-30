//! A reader that fails once more than a fixed number of bytes have been read.

use std::io::{self, Read};

pub(crate) struct CappedReader<R> {
    inner: R,
    remaining: u64,
}

impl<R: Read> CappedReader<R> {
    pub(crate) fn new(inner: R, limit: u64) -> CappedReader<R> {
        CappedReader {
            inner,
            remaining: limit,
        }
    }
}

impl<R: Read> Read for CappedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let room = usize::try_from(self.remaining.saturating_add(1)).unwrap_or(usize::MAX);
        let window = buf.len().min(room);
        let read = match self.inner.read(&mut buf[..window]) {
            Ok(read) => read,
            Err(e) => return Err(e),
        };
        if read as u64 > self.remaining {
            return Err(io::Error::other(
                "bundle is larger than the manifest allows",
            ));
        }
        self.remaining -= read as u64;
        Ok(read)
    }
}
