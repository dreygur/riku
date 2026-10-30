//! `riku dashboard` with a server selected: runs that server's embedded
//! dashboard for this session only and opens it in the local browser.
//!
//! One ssh master to the server's admin login carries every step. On it,
//! one sudo call starts a shell as the deploy user, which reports that it
//! is running, reads the token and starts `riku dashboard --local --bind
//! 127.0.0.1:0`. The session answers sudo's prompt when it shows one, sends
//! the token after the ready marker, reads the port the dashboard bound,
//! forwards a local port to it, and opens the browser. Ctrl-C, SIGHUP,
//! SIGTERM or a dropped link stop that remote process and the master, and
//! leave other sessions alone.
//!
//! Security model: the token is fresh for each session, from the OS CSPRNG,
//! and reaches the server only on stdin, never in an argument list there.
//! The sudo password, when sudo asks for one, takes the same path and is
//! wiped from memory once sent; a rejected password ends the session after
//! one attempt. The remote dashboard binds loopback only, so the ssh forward
//! is the one way in.

mod browser;
mod ended;
mod events;
mod flags;
mod handshake;
mod interrupt;
mod remote_script;
mod session;
mod ssh_master;
mod stop;
mod sudo;

use std::thread::JoinHandle;

use anyhow::{anyhow, Result};

use crate::remote::Server;
use crate::util::token::generate_token;
use session::RemoteDashboard;
use ssh_master::SshControl;

pub use flags::check_remote_flags;
pub use remote_script::BOUND_PREFIX;

const PREFERRED_LOCAL_PORT: u16 = 8088;

/// Parent of the session's private dir. On macOS `TMPDIR` is long enough
/// that ssh's control socket name would pass the 104 byte socket path limit.
const SOCKET_PARENT: &str = "/tmp";

#[allow(clippy::question_mark)]
fn open_forward(ssh: &SshControl, remote_port: u16, token: &str) -> Result<()> {
    let local_port = match browser::pick_local_port(PREFERRED_LOCAL_PORT) {
        Ok(port) => port,
        Err(e) => return Err(e),
    };
    if let Err(e) = ssh.add_forward(local_port, remote_port) {
        return Err(e);
    }
    let url = browser::dashboard_url(local_port, token);
    println!("Dashboard: {url}");
    eprintln!("Press Ctrl-C to stop.");
    browser::open(&url);
    Ok(())
}

#[allow(clippy::question_mark)]
fn drive(ssh: &SshControl, remote: &mut RemoteDashboard, admin_user: &str) -> Result<()> {
    let token = generate_token();
    let remote_port = match remote.start(admin_user, &token) {
        Ok(port) => port,
        Err(e) => return Err(e),
    };
    if let Err(e) = open_forward(ssh, remote_port, &token) {
        return Err(e);
    }
    remote.wait()
}

fn run_session(
    ssh: &SshControl,
    server: &Server,
    admin_user: &str,
) -> (Result<()>, Vec<JoinHandle<()>>) {
    let mut remote = match RemoteDashboard::spawn(ssh, server.address.user()) {
        Ok(remote) => remote,
        Err(e) => return (Err(e), Vec::new()),
    };
    let outcome = drive(ssh, &mut remote, admin_user);
    (outcome, remote.finish())
}

/// A stop signal at any stage is a clean stop, whatever it interrupted.
fn settle(outcome: Result<()>) -> Result<()> {
    match outcome {
        Err(_) if interrupt::requested() => {
            eprintln!("Stopped.");
            Ok(())
        }
        other => other,
    }
}

/// Runs `server`'s dashboard for this session and opens it locally, until
/// Ctrl-C or the remote side ends. `program` is the binary name the user
/// typed, for the fix it suggests when the server has no admin login.
#[allow(clippy::question_mark)]
pub fn run_remote(server: &Server, program: &str) -> Result<()> {
    let name = server.name.as_str();
    let admin = match &server.admin {
        Some(admin) => admin,
        None => {
            return Err(anyhow!(
                "no admin login for {name}: {program} server remove {name}, then {program} server add {name} {} --admin user@host",
                server.address.as_str()
            ))
        }
    };
    if let Err(e) = interrupt::install() {
        return Err(e);
    }
    let dir = match tempfile::Builder::new()
        .prefix("riku-dash-")
        .tempdir_in(SOCKET_PARENT)
    {
        Ok(dir) => dir,
        Err(e) => return Err(anyhow!("could not create a private temp dir: {e}")),
    };
    let ctl = ssh_master::control_path(dir.path());
    let ssh = SshControl::new(&ctl, admin);
    eprintln!("[*] {name} ({})", admin.as_str());
    let master = match ssh.start() {
        Ok(master) => master,
        Err(e) => return settle(Err(e)),
    };
    let (outcome, readers) = run_session(&ssh, server, admin.user());
    drop(master);
    for reader in readers {
        let _ = reader.join();
    }
    settle(outcome)
}
