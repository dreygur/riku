//! The local prompt for the server's sudo password, shown when the remote
//! sudo asks for one.
//!
//! Not unit tested: it drives the real terminal. When to prompt is decided
//! by `handshake`, which is.
//!
//! Security model: the password is read from `/dev/tty` with echo off and
//! the terminal mode is restored on every return, Ctrl-C included. It is
//! read into a buffer allocated once, so no reallocation leaves a copy
//! behind, written from that buffer straight to the remote stdin, and
//! zeroed when dropped. It never goes into an argument list or a log line.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::{compiler_fence, Ordering};

use anyhow::{anyhow, Result};
use nix::sys::termios::{tcgetattr, tcsetattr, LocalFlags, SetArg, Termios};

use super::interrupt;

const MAX_PASSWORD_LEN: usize = 1024;

fn wipe(bytes: &mut [u8]) {
    for byte in bytes.iter_mut() {
        // SAFETY: `byte` is a valid, exclusive reference into the slice.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

/// A password, zeroed when dropped.
pub(super) struct Secret(Vec<u8>);

impl Secret {
    /// Writes the password and a newline to `out` with no intermediate copy.
    pub(super) fn write_line(&self, out: &mut impl Write) -> std::io::Result<()> {
        out.write_all(&self.0)
            .and_then(|()| out.write_all(b"\n"))
            .and_then(|()| out.flush())
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Turns terminal echo off until dropped.
struct EchoOff<'a> {
    tty: &'a File,
    saved: Termios,
}

impl<'a> EchoOff<'a> {
    fn new(tty: &'a File) -> Result<Self> {
        let saved = match tcgetattr(tty) {
            Ok(saved) => saved,
            Err(e) => return Err(anyhow!("could not read the terminal mode: {e}")),
        };
        let mut quiet = saved.clone();
        quiet.local_flags.remove(LocalFlags::ECHO);
        if let Err(e) = tcsetattr(tty, SetArg::TCSANOW, &quiet) {
            return Err(anyhow!("could not turn terminal echo off: {e}"));
        }
        Ok(EchoOff { tty, saved })
    }
}

impl Drop for EchoOff<'_> {
    fn drop(&mut self) {
        let _ = tcsetattr(self.tty, SetArg::TCSANOW, &self.saved);
    }
}

/// One line from the terminal. `None` on end of input (Ctrl-D), which means
/// the user gave up, not that the password is empty.
fn read_secret_line(mut tty: &File) -> Result<Option<Secret>> {
    let mut line = Secret(Vec::with_capacity(MAX_PASSWORD_LEN));
    let mut byte = [0u8; 1];
    let outcome = loop {
        match tty.read(&mut byte) {
            Ok(0) => break Ok(None),
            Ok(_) if byte[0] == b'\n' => break Ok(Some(())),
            Ok(_) if line.0.len() == MAX_PASSWORD_LEN => {
                break Err(anyhow!("password longer than {MAX_PASSWORD_LEN} bytes"))
            }
            Ok(_) => line.0.push(byte[0]),
            Err(e) if e.kind() == ErrorKind::Interrupted => {
                if interrupt::requested() {
                    break Err(anyhow!("interrupted"));
                }
            }
            Err(e) => break Err(anyhow!("could not read the password: {e}")),
        }
    };
    wipe(&mut byte);
    match outcome {
        Ok(Some(())) => Ok(Some(line)),
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Asks for `admin`'s sudo password on the terminal without echo.
#[allow(clippy::question_mark)]
pub(super) fn prompt_password(admin: &str) -> Result<Option<Secret>> {
    let tty = match OpenOptions::new().read(true).write(true).open("/dev/tty") {
        Ok(tty) => tty,
        Err(e) => {
            return Err(anyhow!(
                "sudo needs a password but there is no terminal: {e}"
            ))
        }
    };
    if let Err(e) = (&tty).write_all(format!("[sudo] password for {admin}: ").as_bytes()) {
        return Err(anyhow!("could not write the password prompt: {e}"));
    }
    let echo_off = match EchoOff::new(&tty) {
        Ok(echo_off) => echo_off,
        Err(e) => return Err(e),
    };
    let password = read_secret_line(&tty);
    drop(echo_off);
    let _ = (&tty).write_all(b"\n");
    password
}
