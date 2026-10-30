//! Runs the built `rk` binary against a throwaway config and home directory.
//! Environment is set on the child only, so tests never touch each other.

use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use riku_bundle::signing::verify;
use riku_bundle::PluginManifest;
use tempfile::TempDir;

const NO_SERVER: &str =
    "no server selected; add one with rk server add <name> <user@host>, or pass -s <name>";
const UNREACHABLE: &str = "riku@nonexistent.invalid";

fn write_script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// Puts an `ssh` first on the child's PATH that records its argv and exits
/// 7. Returns the file the argv is recorded in.
fn mock_ssh(home: &TempDir) -> PathBuf {
    let bin = home.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let record = home.path().join("ssh-argv");
    let body = format!("printf '[%s]' \"$@\" > '{}'\nexit 7", record.display());
    write_script(&bin.join("ssh"), &body);
    record
}

fn rk(home: &TempDir, args: &[&str]) -> Output {
    let mut paths = vec![home.path().join("bin")];
    if let Some(parent) = env::var_os("PATH") {
        paths.extend(env::split_paths(&parent));
    }
    let child_path = env::join_paths(paths).expect("PATH entries hold no separator");

    Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(args)
        .env("HOME", home.path())
        .env("PATH", child_path)
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
fn refusal_wins_over_a_help_request() {
    let home = TempDir::new().unwrap();
    for args in [&["init", "-h"][..], &["help", "init"], &["container"]] {
        let output = rk(&home, args);
        assert_eq!(output.status.code(), Some(1), "rk {args:?}");
        assert!(
            stderr(&output).contains("is not part of rk"),
            "rk {args:?}: {}",
            stderr(&output)
        );
    }
}

fn public_key_line(output: &Output) -> String {
    stdout(output)
        .lines()
        .map(str::trim)
        .find(|line| line.len() == 64 && line.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| panic!("no public key in: {}", stdout(output)))
        .to_string()
}

#[test]
fn scaffold_creates_a_bundle_with_no_server_registered() {
    let home = TempDir::new().unwrap();
    let out = rk(&home, &["plugins", "scaffold", "demo", "--type", "runtime"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let bundle = home.path().join("demo");
    assert!(bundle.join("riku-plugin.toml").is_file());
    let mode = fs::metadata(bundle.join("bin/demo"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755);
    assert!(stdout(&out).contains("rk plugins keygen && rk plugins sign"));
    assert!(
        stdout(&out).contains("copy it to the server, then: rk -s <server> plugins install"),
        "{}",
        stdout(&out)
    );
    PluginManifest::from_dir(&bundle).expect("scaffolded manifest should validate");
}

#[test]
fn keygen_writes_a_private_key_and_prints_the_public_key() {
    let home = TempDir::new().unwrap();
    let ssh_record = mock_ssh(&home);
    let added = rk(&home, &["server", "add", "t", UNREACHABLE]);
    assert!(added.status.success(), "{}", stderr(&added));

    let key = home.path().join("k");
    let out = rk(
        &home,
        &["plugins", "keygen", "--out", key.to_str().unwrap()],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        fs::metadata(&key).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(public_key_line(&out).len(), 64);
    assert!(stdout(&out).contains("rk -s <server> plugins trust add <name> <pubkey>"));
    assert!(!ssh_record.exists(), "keygen must not reach a server");
}

#[test]
fn sign_writes_a_signature_the_bundle_crate_verifies() {
    let home = TempDir::new().unwrap();
    let scaffold = rk(&home, &["plugins", "scaffold", "demo", "--type", "addon"]);
    assert!(scaffold.status.success(), "{}", stderr(&scaffold));
    let key = home.path().join("k");
    let keygen = rk(
        &home,
        &["plugins", "keygen", "--out", key.to_str().unwrap()],
    );
    let bundle = home.path().join("demo");

    let out = rk(
        &home,
        &[
            "plugins",
            "sign",
            bundle.to_str().unwrap(),
            "--key",
            key.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let manifest = PluginManifest::from_dir(&bundle).unwrap();
    let signature = manifest
        .signature
        .as_deref()
        .expect("manifest has a signature");
    let entry = fs::read(manifest.entry_path(&bundle)).unwrap();
    assert!(verify(&entry, signature, &public_key_line(&keygen)));
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
fn flag_forwards_to_ssh_with_the_server_flag_stripped() {
    let home = TempDir::new().unwrap();
    let record = mock_ssh(&home);
    let added = rk(&home, &["server", "add", "t", UNREACHABLE]);
    assert!(added.status.success(), "{}", stderr(&added));

    let output = rk(&home, &["-s", "t", "run", "myapp", "echo", "a b"]);
    assert_eq!(output.status.code(), Some(7), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("[*] t (riku@nonexistent.invalid)"),
        "{}",
        stderr(&output)
    );
    let recorded = fs::read_to_string(&record).expect("mock ssh should record its argv");
    assert_eq!(
        recorded,
        "[-t][--][riku@nonexistent.invalid][run myapp echo 'a b']"
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
fn client_plugin_runs_with_an_empty_server_when_none_is_selected() {
    let home = TempDir::new().unwrap();
    let plugins = home.path().join(".riku").join("client-plugins");
    fs::create_dir_all(&plugins).unwrap();
    let marker = home.path().join("ran");
    let body = format!("printf '[%s]' \"$@\" > '{}'", marker.display());
    write_script(&plugins.join("apps"), &body);

    let output = rk(&home, &["apps"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let args = fs::read_to_string(&marker).expect("mock client plugin should run");
    assert_eq!(args, "[][][apps]", "server, app and command the plugin got");
}
