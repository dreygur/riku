//! A reader that fails once its byte budget is spent, and the shared budget.

use std::cell::Cell;
use std::io::{self, Read};
use std::rc::Rc;

/// Bytes a reader may still deliver. Clones share one counter, so the code that
/// owns the stream can raise or reset the allowance while a library holds the
/// reader.
#[derive(Clone)]
pub(crate) struct Budget(Rc<Cell<u64>>);

impl Budget {
    pub(crate) fn new(bytes: u64) -> Budget {
        Budget(Rc::new(Cell::new(bytes)))
    }

    pub(crate) fn allow(&self, bytes: u64) {
        self.0.set(self.0.get().saturating_add(bytes));
    }

    /// Lowers the allowance to `bytes` if more than that is left.
    pub(crate) fn cap_at(&self, bytes: u64) {
        self.0.set(self.0.get().min(bytes));
    }
}

pub(crate) struct CappedReader<R> {
    inner: R,
    budget: Budget,
}

impl<R: Read> CappedReader<R> {
    pub(crate) fn new(inner: R, budget: Budget) -> CappedReader<R> {
        CappedReader { inner, budget }
    }
}

impl<R: Read> Read for CappedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let remaining = self.budget.0.get();
        let room = usize::try_from(remaining.saturating_add(1)).unwrap_or(usize::MAX);
        let window = buf.len().min(room);
        let read = match self.inner.read(&mut buf[..window]) {
            Ok(read) => read,
            Err(e) => return Err(e),
        };
        if read as u64 > remaining {
            return Err(io::Error::other(
                "bundle is larger than the manifest allows",
            ));
        }
        self.budget.0.set(remaining - read as u64);
        Ok(read)
    }
}
