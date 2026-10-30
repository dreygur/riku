//! Runs the built `rk` binary against a throwaway config and home directory.
//! Environment is set on the child only, so tests never touch each other.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};

use tempfile::TempDir;

const NO_SERVER: &str =
    "no server selected; add one with rk server add <name> <user@host>, or pass -s <name>";
const UNREACHABLE: &str = "riku@nonexistent.invalid";

fn rk(home: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(args)
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("config"))
        .env_remove("SSH_ORIGINAL_COMMAND")
        .current_dir(home.path())
        .output()
        .expect("rk should start")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn forwarded_command_without_a_server_is_an_error() {
    let home = TempDir::new().unwrap();
    let output = rk(&home, &["apps"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains(NO_SERVER), "{}", stderr(&output));
}

#[test]
fn added_servers_are_listed_and_named_in_the_no_server_error() {
    let home = TempDir::new().unwrap();
    let added = rk(&home, &["server", "add", "t", UNREACHABLE]);
    assert!(added.status.success(), "{}", stderr(&added));
    let listed = rk(&home, &["server", "list"]);
    assert!(stdout(&listed).contains(UNREACHABLE), "{}", stdout(&listed));
    let output = rk(&home, &["apps"]);
    assert!(
        stderr(&output).contains("registered: t"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn server_only_commands_and_container_are_refused() {
    let home = TempDir::new().unwrap();
    let init = rk(&home, &["init"]);
    assert_eq!(init.status.code(), Some(1));
    assert!(
        stderr(&init).contains("'init' is not part of rk; run it with riku on the server"),
        "{}",
        stderr(&init)
    );
    let container = rk(&home, &["container", "export", "myapp"]);
    assert_eq!(container.status.code(), Some(1));
    assert!(
        stderr(&container)
            .contains("'container' is not part of rk; run it with the full riku binary"),
        "{}",
        stderr(&container)
    );
}

#[test]
fn plugin_authoring_is_refused_naming_the_subcommand() {
    let home = TempDir::new().unwrap();
    let out = rk(&home, &["plugins", "keygen"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out)
            .contains("'plugins keygen' is not part of rk; run it with the full riku binary"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn local_flag_is_refused() {
    let home = TempDir::new().unwrap();
    let output = rk(&home, &["--local", "apps"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("no local mode"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn flag_forwards_over_ssh_and_names_the_target() {
    let home = TempDir::new().unwrap();
    assert!(rk(&home, &["server", "add", "t", UNREACHABLE])
        .status
        .success());
    let output = rk(&home, &["-s", "t", "apps"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("[*] t (riku@nonexistent.invalid)"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn help_lists_what_rk_runs_and_hides_the_rest() {
    let home = TempDir::new().unwrap();
    let output = rk(&home, &["-h"]);
    let text = stdout(&output);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(text.contains("\n  server "), "{text}");
    assert!(text.contains("\n  apps "), "{text}");
    assert!(!text.contains("\n  init "), "{text}");
}

#[test]
fn client_plugin_runs_before_the_no_server_error() {
    let home = TempDir::new().unwrap();
    let plugins = home.path().join(".riku").join("client-plugins");
    fs::create_dir_all(&plugins).unwrap();
    let plugin = plugins.join("apps");
    let marker = home.path().join("ran");
    let script = format!("#!/bin/sh\necho \"$@\" > {}\n", marker.display());
    fs::write(&plugin, script).unwrap();
    fs::set_permissions(&plugin, fs::Permissions::from_mode(0o755)).unwrap();

    let output = rk(&home, &["apps"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(marker.exists(), "mock client plugin did not run");
}
