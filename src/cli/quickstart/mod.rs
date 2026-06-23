//! `riku quickstart` — scaffold a runnable sample app and print the exact
//! `git remote add` / `git push` lines, so a new user deploys in minutes.
//!
//! This runs on the *developer's* machine (it writes into the current
//! directory and `git init`s), independent of the server-side install.

mod scaffold;

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};

use crate::util::display;

use scaffold::Runtime;

/// Scaffold `name` (a new directory) for `runtime`, git-init it, and print
/// deploy instructions. `remote` fills the `git remote add` line when known.
pub fn cmd_quickstart(name: &str, runtime: &str, remote: Option<&str>) -> Result<()> {
    let app = crate::util::validate_app_name(name)?;
    let rt = Runtime::parse(runtime)?;

    let dir = Path::new(&app);
    if dir.exists() {
        bail!("'{app}' already exists here — choose another name or remove it");
    }

    display::info(&format!("Scaffolding {} app in ./{app}...", rt.label()));
    std::fs::create_dir_all(dir)?;
    for file in rt.files(&app) {
        let path = dir.join(file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &file.contents)?;
        display::note(&format!("created {app}/{}", file.path));
    }

    let committed = git_init_commit(dir);

    print_next_steps(&app, rt.label(), remote, committed);
    Ok(())
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

    // One test owns the process-global cwd change, so it never races another
    // chdir-ing test under a threaded runner.
    #[test]
    fn scaffolds_new_dir_and_refuses_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let scaffolded = cmd_quickstart("demo-app", "python", Some("deploy@host"));
        let app_dir = tmp.path().join("demo-app");
        let scaffold_ok = scaffolded.is_ok()
            && app_dir.join("Procfile").exists()
            && app_dir.join("app.py").exists()
            && app_dir.join("requirements.txt").exists();

        std::fs::create_dir(tmp.path().join("taken")).unwrap();
        let refused = cmd_quickstart("taken", "python", None).is_err();

        std::env::set_current_dir(prev).unwrap();
        assert!(scaffold_ok, "expected a scaffolded python app");
        assert!(refused, "must not overwrite an existing directory");
    }
}
