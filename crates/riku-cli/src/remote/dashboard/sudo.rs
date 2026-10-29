//! Sudo access on the server for the dashboard session: the non-interactive
//! probe, and the local password prompt when sudo asks for one.
//!
//! Not unit tested: the probe needs a reachable host and the prompt drives
//! the real terminal. The decision to prompt is `needs_password`, which is.
//!
//! Security model: the password is read from `/dev/tty` with echo off and
//! the terminal mode is restored on every return, Ctrl-C included. It is
//! held in a `Secret`, which zeroes its bytes when dropped, and it never
//! goes into an argument list or a log line.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{compiler_fence, Ordering};

use anyhow::{anyhow, Result};
use nix::sys::termios::{tcgetattr, tcsetattr, LocalFlags, SetArg, Termios};

use super::interrupt;
use super::remote_script::{needs_password, probe_command, SudoMode};
use super::ssh_master::SshControl;

/// Allocated up front so the buffer never reallocates and leaves a copy behind.
const MAX_PASSWORD_LEN: usize = 1024;

fn wipe(bytes: &mut [u8]) {
    for byte in bytes.iter_mut() {
        // SAFETY: `byte` is a valid, exclusive reference into the slice.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

/// Bytes that are zeroed when dropped.
pub(super) struct Secret(Vec<u8>);

impl Secret {
    pub(super) fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub(super) fn as_str(&self) -> Result<&str> {
        match std::str::from_utf8(&self.0) {
            Ok(text) => Ok(text),
            Err(_) => Err(anyhow!("the sudo password is not valid UTF-8")),
        }
    }
}

impl From<String> for Secret {
    fn from(text: String) -> Self {
        Secret(text.into_bytes())
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

fn read_secret_line(mut tty: &File) -> Result<Secret> {
    let mut line = Secret(Vec::with_capacity(MAX_PASSWORD_LEN));
    let mut byte = [0u8; 1];
    loop {
        match tty.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) if line.0.len() == MAX_PASSWORD_LEN => {
                return Err(anyhow!("password longer than {MAX_PASSWORD_LEN} bytes"))
            }
            Ok(_) => line.0.push(byte[0]),
            Err(e) if e.kind() == ErrorKind::Interrupted => {
                if interrupt::requested() {
                    return Err(anyhow!("interrupted"));
                }
            }
            Err(e) => return Err(anyhow!("could not read the password: {e}")),
        }
    }
    wipe(&mut byte);
    Ok(line)
}

/// Asks for `admin`'s sudo password on the terminal without echo.
#[allow(clippy::question_mark)]
pub(super) fn prompt_password(admin: &str) -> Result<Secret> {
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

/// Whether the admin may become `deploy` without a password. Any sudo
/// failure other than a missing password ends the session with sudo's text.
pub(super) fn probe(ssh: &SshControl, deploy: &str) -> Result<SudoMode> {
    let output = match Command::new("ssh")
        .args(ssh.command_args(&probe_command(deploy)))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("failed to start ssh: {e}")),
    };
    if output.status.success() {
        return Ok(SudoMode::NoPassword);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if needs_password(&stderr) {
        return Ok(SudoMode::Password);
    }
    Err(anyhow!("sudo to {deploy} failed: {}", stderr.trim()))
}
