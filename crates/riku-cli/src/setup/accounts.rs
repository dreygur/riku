//! The account changes root init makes: create the deploy user, create a
//! group, add a user to a group, and (shadow-utils only, through
//! [`super::deploy_account`]) move the deploy user to `/bin/sh`. This
//! module picks the tool for the host.
//!
//! shadow-utils (`useradd`, `groupadd`, `usermod`) is used when all three
//! are on `PATH`, which covers Debian, Ubuntu, Fedora, RHEL, Arch and
//! openSUSE. Without it, BusyBox's `adduser`, `addgroup` and `chpasswd`,
//! which Alpine ships. NixOS builds its accounts from configuration.nix, so
//! there riku changes nothing and prints the lines to add instead, which
//! stops init until the operator rebuilds and runs it again. With neither
//! tool set, init stops naming what to install.
//!
//! Security: callers check every account and group name before it gets
//! here (`AccountName`, `socket_group::is_account_name`), and every tool
//! runs with a fixed argv and no shell. `PATH` decides only which tool set
//! is used; a relative `PATH` entry is ignored.

use std::ffi::OsStr;
use std::fmt;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use nix::unistd::Group;

use crate::config::SOCKET_GROUP;

const SHADOW_TOOLS: [&str; 3] = ["useradd", "groupadd", "usermod"];
const BUSYBOX_TOOLS: [&str; 3] = ["adduser", "addgroup", "chpasswd"];
const NIXOS_MARKER: &str = "/etc/NIXOS";
const OS_RELEASE: &str = "/etc/os-release";
/// An unmatchable password hash. `useradd` leaves `!` (locked), and OpenSSH
/// refuses every auth method for a locked account, pubkey included.
const NO_PASSWORD: &str = "*";
const HOME_ROOT: &str = "/home";
const REBUILD: &str = "Add these lines to configuration.nix, run nixos-rebuild switch, \
     then run sudo riku init again. riku changes no users or groups on NixOS";

/// The tool set that changes accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tool {
    Shadow,
    BusyBox,
}

/// How this host manages accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Host {
    Tools(Tool),
    NixOs,
    NoTools,
}

/// One command: a program, its arguments, and what to write on its stdin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Cmd {
    pub program: &'static str,
    pub args: Vec<String>,
    pub input: Option<String>,
}

impl Cmd {
    pub(super) fn new(program: &'static str, args: &[&str]) -> Self {
        Cmd {
            program,
            args: args.iter().map(|arg| arg.to_string()).collect(),
            input: None,
        }
    }
}

/// The command line as an operator would type it.
impl fmt::Display for Cmd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.program, self.args.join(" "))
    }
}

fn on_path(search_path: &OsStr, tool: &str) -> bool {
    std::env::split_paths(search_path)
        .filter(|dir| dir.is_absolute())
        .any(|dir| dir.join(tool).is_file())
}

fn names_nixos(os_release: &str) -> bool {
    os_release
        .lines()
        .filter_map(|line| line.strip_prefix("ID="))
        .any(|id| id.trim_matches('"') == "nixos")
}

/// The host kind for `search_path`, the text of `/etc/os-release` and
/// whether `/etc/NIXOS` exists.
pub(super) fn host_kind(search_path: &OsStr, os_release: &str, nixos_marker: bool) -> Host {
    if nixos_marker || names_nixos(os_release) {
        return Host::NixOs;
    }
    let all_on_path = |tools: &[&str]| tools.iter().all(|tool| on_path(search_path, tool));
    if all_on_path(&SHADOW_TOOLS) {
        Host::Tools(Tool::Shadow)
    } else if all_on_path(&BUSYBOX_TOOLS) {
        Host::Tools(Tool::BusyBox)
    } else {
        Host::NoTools
    }
}

pub(super) fn this_host() -> Host {
    let search_path = std::env::var_os("PATH").unwrap_or_default();
    let os_release = std::fs::read_to_string(OS_RELEASE).unwrap_or_default();
    host_kind(&search_path, &os_release, Path::new(NIXOS_MARKER).exists())
}

pub(super) fn no_tools_message() -> String {
    format!(
        "no tool to create users and groups: install shadow ({}) or busybox ({})",
        SHADOW_TOOLS.join(", "),
        BUSYBOX_TOOLS.join(", ")
    )
}

/// The commands that create login account `name` with its own group, a
/// home under `/home`, `shell`, and the unmatchable password `*`.
pub(super) fn create_user_commands(tool: Tool, name: &str, shell: &str) -> Vec<Cmd> {
    match tool {
        Tool::Shadow => vec![Cmd::new(
            "useradd",
            &[
                "--create-home",
                "--user-group",
                "--shell",
                shell,
                "--password",
                NO_PASSWORD,
                "--",
                name,
            ],
        )],
        Tool::BusyBox => {
            let home = format!("{HOME_ROOT}/{name}");
            let mut set_password = Cmd::new("chpasswd", &["-e"]);
            set_password.input = Some(format!("{name}:{NO_PASSWORD}\n"));
            vec![
                Cmd::new("addgroup", &["-S", name]),
                Cmd::new(
                    "adduser",
                    &["-S", "-D", "-h", &home, "-s", shell, "-G", name, name],
                ),
                set_password,
            ]
        }
    }
}

pub(super) fn create_group_command(tool: Tool, group: &str) -> Cmd {
    match tool {
        Tool::Shadow => Cmd::new("groupadd", &["--system", group]),
        Tool::BusyBox => Cmd::new("addgroup", &["-S", group]),
    }
}

pub(super) fn add_to_group_command(tool: Tool, user: &str, group: &str) -> Cmd {
    match tool {
        Tool::Shadow => Cmd::new("usermod", &["-aG", group, user]),
        Tool::BusyBox => Cmd::new("addgroup", &[user, group]),
    }
}

/// The configuration.nix lines for deploy account `user`: the account, its
/// own group, and the socket group it belongs to.
pub(super) fn nixos_account_lines(user: &str) -> String {
    format!(
        "users.groups.{user} = {{ }};\n\
         users.groups.{SOCKET_GROUP} = {{ }};\n\
         users.users.{user} = {{\n  \
           isNormalUser = true;\n  \
           group = \"{user}\";\n  \
           home = \"{HOME_ROOT}/{user}\";\n  \
           createHome = true;\n  \
           shell = pkgs.bash;\n  \
           hashedPassword = \"{NO_PASSWORD}\";\n  \
           extraGroups = [ \"{SOCKET_GROUP}\" ];\n\
         }};"
    )
}

pub(super) fn nixos_group_line(user: &str, group: &str) -> String {
    format!("users.users.{user}.extraGroups = [ \"{group}\" ];")
}

fn nixos_stop(lines: &str) -> anyhow::Error {
    anyhow!("{REBUILD}:\n{lines}")
}

#[allow(clippy::question_mark)]
pub(super) fn run(cmd: &Cmd) -> Result<()> {
    let mut command = Command::new(cmd.program);
    command.args(&cmd.args);
    let stdin = if cmd.input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    command.stdin(stdin);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => return Err(anyhow!("failed to run {cmd}: {e}")),
    };
    if let (Some(input), Some(mut stdin)) = (&cmd.input, child.stdin.take()) {
        if let Err(e) = stdin.write_all(input.as_bytes()) {
            return Err(anyhow!("failed to write to {}: {e}", cmd.program));
        }
    }
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => return Err(anyhow!("failed to run {cmd}: {e}")),
    };
    if !output.status.success() {
        return Err(anyhow!(
            "{cmd} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn run_all(cmds: &[Cmd]) -> Result<()> {
    cmds.iter().try_for_each(run)
}

/// Create login account `name` with `shell`. On NixOS, the error carries
/// the configuration.nix lines and stops init.
pub(super) fn create_user(name: &str, shell: &str) -> Result<()> {
    match this_host() {
        Host::Tools(tool) => run_all(&create_user_commands(tool, name, shell)),
        Host::NixOs => Err(nixos_stop(&nixos_account_lines(name))),
        Host::NoTools => Err(anyhow!(no_tools_message())),
    }
}

/// Create system group `group`. On NixOS, the error carries the
/// configuration.nix line and stops init.
pub(super) fn create_group(group: &str) -> Result<()> {
    match this_host() {
        Host::Tools(tool) => run(&create_group_command(tool, group)),
        Host::NixOs => Err(nixos_stop(&format!("users.groups.{group} = {{ }};"))),
        Host::NoTools => Err(anyhow!(no_tools_message())),
    }
}

fn is_member(user: &str, group: &str) -> bool {
    matches!(Group::from_name(group), Ok(Some(found)) if found.mem.iter().any(|m| m == user))
}

/// Add `user` to `group`, which must exist. A failed tool's error ends with
/// the command to run by hand. On NixOS a member already is left alone,
/// and anyone else gets the configuration.nix line in the error.
pub(super) fn add_to_group(user: &str, group: &str) -> Result<()> {
    match this_host() {
        Host::Tools(tool) => {
            let cmd = add_to_group_command(tool, user, group);
            match run(&cmd) {
                Ok(()) => Ok(()),
                Err(e) => Err(anyhow!("{e:#}. Run it yourself: {cmd}")),
            }
        }
        Host::NixOs if is_member(user, group) => Ok(()),
        Host::NixOs => Err(nixos_stop(&nixos_group_line(user, group))),
        Host::NoTools => Err(anyhow!(no_tools_message())),
    }
}

#[cfg(test)]
#[path = "accounts_tests.rs"]
mod tests;
