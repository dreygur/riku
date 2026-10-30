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

    display::info(&format!("Scaffolding {} app in ./{app}...", rt.label()));
    for file in rt.files(&app) {
        if let Err(e) = write_file(&dir.join(file.path), &file.contents) {
            return Err(e);
        }
        display::note(&format!("created {app}/{}", file.path));
    }

    let committed = git_init_commit(&dir);

    print_next_steps(&app, rt.label(), remote, committed);
    Ok(())
}

/// Scaffold `name` (a new directory in the current directory) for
/// `runtime`, git-init it, and print deploy instructions. `remote` fills the
/// `git remote add` line when known.
pub fn cmd_quickstart(name: &str, runtime: &str, remote: Option<&str>) -> Result<()> {
    scaffold_into(Path::new("."), name, runtime, remote)
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

fn print_next_steps(app: &str, runtime: &str, remote: Option<&str>, committed: bool) {
    let target = remote
        .map(|r| format!("{r}:{app}"))
        .unwrap_or_else(|| format!("<user>@<your-server>:{app}"));

    display::blank();
    display::success(&format!("{runtime} app '{app}' is ready."));
    display::blank();
    display::section("Deploy it");

    let mut step = 1;
    println!("  {step}. cd {app}");
    step += 1;
    if !committed {
        println!("  {step}. git init && git add -A && git commit -m 'init'");
        step += 1;
    }
    println!("  {step}. git remote add riku {target}");
    step += 1;
    println!("  {step}. git push riku main");

    display::blank();
    if remote.is_none() {
        display::note("Replace <user>@<your-server> with your Riku host (see `riku init`).");
    }
    display::note("On push, Riku detects the runtime, builds, and serves the app on $PORT.");
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
