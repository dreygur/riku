//! `riku quickstart`: scaffold a runnable sample app and print the exact
//! `git remote add` / `git push` lines, so a new user deploys in minutes.
//!
//! This runs on the *developer's* machine (it writes into the current
//! directory and `git init`s), independent of the server-side install.

mod scaffold;

use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, bail, Result};

use crate::util::display;

use scaffold::Runtime;

fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Err(anyhow!("failed to create {}: {e}", parent.display()));
        }
    }
    match std::fs::write(path, contents) {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow!("failed to write {}: {e}", path.display())),
    }
}

/// Scaffold `name` under `base`. Taking the directory instead of using the
/// process cwd keeps tests from changing a cwd every other test thread and
/// its child processes share.
#[allow(clippy::question_mark)]
fn scaffold_into(base: &Path, name: &str, runtime: &str, remote: Option<&str>) -> Result<()> {
    let app = match crate::util::validate_app_name(name) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    let rt = match Runtime::parse(runtime) {
        Ok(rt) => rt,
        Err(e) => return Err(e),
    };

    let dir = base.join(&app);
    if dir.exists() {
        bail!("'{app}' already exists here, choose another name or remove it");
    }

    display::intro(&format!("riku quickstart  {app} ({})", rt.label()));
    let files = rt.files(&app);
    for file in &files {
        if let Err(e) = write_file(&dir.join(file.path), &file.contents) {
            return Err(e);
        }
    }
    display::step_done("Files", &format!("{} in ./{app}", files.len()));
    for file in &files {
        display::detail(&format!("{app}/{}", file.path));
    }

    let committed = git_init_commit(&dir);
    if committed {
        display::step_done("Git", "initial commit");
    } else {
        display::warn("Git repository not created, commit it yourself");
    }

    print_next_steps(&app, rt.label(), remote, committed);
    Ok(())
}

/// The default registered server's address. An unreadable list counts as
/// none: the instructions then say `rk server add`.
fn default_server_address() -> Option<String> {
    let list = crate::remote::store::servers_path()
        .ok()
        .and_then(|path| crate::remote::store::load(&path).ok());
    list.and_then(|list| {
        list.default_server()
            .map(|s| s.address.as_str().to_string())
    })
}

/// Scaffold `name` (a new directory in the current directory) for
/// `runtime`, git-init it, and print deploy instructions. `remote`, else
/// the default registered server, fills the `git remote add` line.
pub fn cmd_quickstart(name: &str, runtime: &str, remote: Option<&str>) -> Result<()> {
    let remote = remote.map(str::to_string).or_else(default_server_address);
    scaffold_into(Path::new("."), name, runtime, remote.as_deref())
}

/// Best-effort `git init` + initial commit. Returns whether a commit was made.
/// User identity is supplied inline so it works without global git config.
fn git_init_commit(dir: &Path) -> bool {
    if Command::new("git").arg("--version").output().is_err() {
        return false;
    }
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    if !git(&["init", "-q"]) || !git(&["add", "-A"]) {
        return false;
    }
    git(&[
        "-c",
        "user.name=riku",
        "-c",
        "user.email=riku@localhost",
        "commit",
        "-q",
        "-m",
        "Initial commit (riku quickstart)",
    ])
}

/// The step that points the repo at a server: the given remote, else
/// `rk server add`, which also authorizes the key and names the remote
/// after the directory, the app name here.
fn remote_step(app: &str, remote: Option<&str>) -> String {
    match remote {
        Some(remote) => format!("git remote add riku {remote}:{app}"),
        None => "rk server add <name> riku@<host> --admin <login>@<host>".to_string(),
    }
}

/// The commands that deploy the new app, in order, as summary rows.
fn next_steps(app: &str, remote: Option<&str>, committed: bool) -> Vec<(&'static str, String)> {
    let mut rows = vec![("Enter", format!("cd {app}"))];
    if !committed {
        rows.push((
            "Commit",
            "git init && git add -A && git commit -m 'init'".to_string(),
        ));
    }
    rows.push(("Remote", remote_step(app, remote)));
    rows.push(("Deploy", "git push riku main".to_string()));
    rows
}

fn print_next_steps(app: &str, runtime: &str, remote: Option<&str>, committed: bool) {
    if remote.is_none() {
        display::detail("rk server add authorizes your ssh key and adds the riku git remote.");
    }
    display::detail("On push, Riku detects the runtime, builds, and serves the app on $PORT.");
    display::outro(&format!("{runtime} app {app} is ready"));
    let rows = next_steps(app, remote, committed);
    let pairs: Vec<(&str, &str)> = rows.iter().map(|(k, v)| (*k, v.as_str())).collect();
    display::summary(&pairs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffolds_new_dir_and_refuses_existing() {
        let tmp = tempfile::tempdir().unwrap();

        let scaffolded = scaffold_into(tmp.path(), "demo-app", "python", Some("riku@host"));
        let app_dir = tmp.path().join("demo-app");
        assert!(scaffolded.is_ok(), "scaffold failed: {scaffolded:?}");
        for file in ["Procfile", "app.py", "requirements.txt"] {
            assert!(app_dir.join(file).exists(), "missing {file}");
        }
        assert!(
            app_dir.join(".git").exists() || Command::new("git").arg("--version").output().is_err(),
            "expected a git repo in the scaffold"
        );

        std::fs::create_dir(tmp.path().join("taken")).unwrap();
        let refused = scaffold_into(tmp.path(), "taken", "python", None).is_err();
        assert!(refused, "must not overwrite an existing directory");
    }
}
