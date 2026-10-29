//! `riku dashboard` with a server selected: runs that server's embedded
//! dashboard for this session only and opens it in the local browser.
//!
//! One ssh master to the server's admin login carries every step. On it,
//! sudo starts `riku dashboard --bind 127.0.0.1:0` as the deploy user, the
//! session reads the port it bound, forwards a local port to it, and opens
//! the browser. Ctrl-C or a dropped link stops that remote process and the
//! master, and leaves other sessions alone.
//!
//! Security model: the token is fresh for each session, from the OS CSPRNG,
//! and reaches the server only on stdin, never in an argument list there.
//! The sudo password, when sudo wants one, takes the same path and is wiped
//! from memory once sent. The remote dashboard binds loopback only, so the
//! ssh forward is the one way in.

mod browser;
mod interrupt;
mod remote_script;
mod session;
mod ssh_master;
mod sudo;

use anyhow::{anyhow, Result};

use crate::remote::Server;
use crate::supervisor::health::auth::generate_token;
use remote_script::{payload, SudoMode};
use session::{Ended, RemoteDashboard};
use ssh_master::SshControl;
use sudo::Secret;

pub use remote_script::BOUND_PREFIX;

const PREFERRED_LOCAL_PORT: u16 = 8088;

/// Parent of the session's private dir. On macOS `TMPDIR` is long enough
/// that ssh's control socket name would pass the 104 byte socket path limit.
const SOCKET_PARENT: &str = "/tmp";

fn sudo_password(mode: SudoMode, ssh_admin: &str) -> Result<Option<Secret>> {
    match mode {
        SudoMode::NoPassword => Ok(None),
        SudoMode::Password => sudo::prompt_password(ssh_admin).map(Some),
    }
}

#[allow(clippy::question_mark)]
fn send_credentials(remote: &mut RemoteDashboard, password: Option<Secret>) -> Result<String> {
    let token = generate_token();
    let password_text = match password.as_ref().map(Secret::as_str) {
        None => None,
        Some(Ok(text)) => Some(text),
        Some(Err(e)) => return Err(e),
    };
    let sent = Secret::from(payload(password_text, &token));
    match remote.send(&sent) {
        Ok(()) => Ok(token),
        Err(e) => Err(e),
    }
}

#[allow(clippy::question_mark)]
fn open_forward(ssh: &SshControl, remote: &mut RemoteDashboard, token: &str) -> Result<()> {
    let remote_port = match remote.bound_port() {
        Ok(port) => port,
        Err(e) => return Err(e),
    };
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
fn drive(ssh: &SshControl, remote: &mut RemoteDashboard, password: Option<Secret>) -> Result<()> {
    let token = match send_credentials(remote, password) {
        Ok(token) => token,
        Err(e) => return Err(e),
    };
    if let Err(e) = open_forward(ssh, remote, &token) {
        return Err(e);
    }
    match remote.wait() {
        Ok(Ended::Interrupted) => Ok(()),
        Ok(Ended::RemoteExited(status)) => Err(anyhow!("the remote dashboard ended ({status})")),
        Err(e) => Err(e),
    }
}

#[allow(clippy::question_mark)]
fn run_session(ssh: &SshControl, server: &Server, admin_user: &str) -> Result<()> {
    let deploy = server.address.user();
    let mode = match sudo::probe(ssh, deploy) {
        Ok(mode) => mode,
        Err(e) => return Err(e),
    };
    let password = match sudo_password(mode, admin_user) {
        Ok(password) => password,
        Err(e) => return Err(e),
    };
    let mut remote = match RemoteDashboard::spawn(ssh, mode, deploy) {
        Ok(remote) => remote,
        Err(e) => return Err(e),
    };
    let outcome = drive(ssh, &mut remote, password);
    remote.finish();
    outcome
}

/// Runs `server`'s dashboard for this session and opens it locally, until
/// Ctrl-C or the remote side ends.
#[allow(clippy::question_mark)]
pub fn run_remote(server: &Server) -> Result<()> {
    let name = server.name.as_str();
    let admin = match &server.admin {
        Some(admin) => admin,
        None => {
            return Err(anyhow!(
                "no admin login for {name}, add one: riku server add {name} {} --admin user@host",
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
        Err(e) => return Err(e),
    };
    let outcome = run_session(&ssh, server, admin.user());
    ssh.stop(master);
    outcome
}
