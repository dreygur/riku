//! What a deploy prints, on the rail from `riku_util::display`: an intro
//! naming the app and commit, one step per phase, then an outro and a
//! summary. A failed deploy names the phase that failed, repeats the last
//! lines of build output and says how to retry.
//!
//! Every line also lands in the deploy log through [`DeployLogger`].

use std::time::{Duration, Instant};

use crate::config::RikuPaths;
use crate::git_ops::Commit;
use crate::releases::ReleaseLog;
use crate::util::deploy_logger::DeployLogger;
use crate::util::display::{self, OutputTail};

/// A deploy phase, as named in its step line and in a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Sync,
    Config,
    Preflight,
    Runtime,
    Build,
    Release,
    Processes,
    Route,
}

impl Phase {
    fn label(self) -> &'static str {
        match self {
            Phase::Sync => "Sync",
            Phase::Config => "Config",
            Phase::Preflight => "Preflight",
            Phase::Runtime => "Runtime",
            Phase::Build => "Build",
            Phase::Release => "Release",
            Phase::Processes => "Processes",
            Phase::Route => "Route",
        }
    }

    /// Whether this phase streams command output worth repeating on failure.
    fn streams_output(self) -> bool {
        matches!(self, Phase::Preflight | Phase::Build | Phase::Release)
    }

    /// Whether the app's running processes are still the previous release
    /// when this phase fails: nothing is restarted before `Processes`.
    fn keeps_previous_release(self) -> bool {
        !matches!(self, Phase::Processes | Phase::Route)
    }
}

/// The facts the success summary lists.
pub(crate) struct Outcome<'a> {
    pub commit: Option<&'a Commit>,
    pub urls: &'a [String],
    pub workers: &'a str,
}

/// The running output of one deploy.
pub(crate) struct DeployProgress {
    pub(crate) log: DeployLogger,
    app: String,
    started: Instant,
    phase: Phase,
    phase_started: Instant,
    output: OutputTail,
    had_release: bool,
}

impl DeployProgress {
    pub(crate) fn new(app: &str, paths: &RikuPaths, log: DeployLogger) -> Self {
        let now = Instant::now();
        Self {
            log,
            app: app.to_string(),
            started: now,
            phase: Phase::Sync,
            phase_started: now,
            output: OutputTail::default(),
            had_release: !ReleaseLog::new(paths).list(app).is_empty(),
        }
    }

    /// `┌  Deploying <app>  <sha> "<subject>"`.
    pub(crate) fn intro(&mut self, commit: Option<&Commit>) {
        let title = intro_title(&self.app, commit);
        display::intro(&title);
        self.log.log_raw(&title);
    }

    /// Start timing `phase`; a failure from here on is reported as it.
    pub(crate) fn begin(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_started = Instant::now();
    }

    /// `◇  <phase>  <detail>`, the current phase finished.
    pub(crate) fn done(&mut self, detail: &str) {
        let label = self.phase.label();
        self.log.step(label, detail);
    }

    /// Like [`Self::done`], with the phase's duration after `detail`.
    pub(crate) fn done_timed(&mut self, detail: &str) {
        let took = format_duration(self.phase_started.elapsed());
        let detail = if detail.is_empty() {
            took
        } else {
            format!("{detail}  {took}")
        };
        self.done(&detail);
    }

    /// Where the build and release commands keep their last lines.
    pub(crate) fn output(&self) -> &OutputTail {
        &self.output
    }

    /// The outro and summary of a finished deploy.
    pub(crate) fn succeeded(&mut self, outcome: &Outcome) {
        let took = format_duration(self.started.elapsed());
        display::outro(&format!("Deployed in {took}"));
        self.log
            .log_raw(&format!("Deploy of '{}' complete", self.app));
        let rows = success_summary(&self.app, outcome);
        display::summary(&as_pairs(&rows));
    }

    /// The failed phase with `err`, the last build lines, the outro and the
    /// summary that says how to retry.
    pub(crate) fn failed(&mut self, err: &anyhow::Error, branch: &str) {
        let reason = format!("{err:#}");
        self.log.log_raw(&format!("[ERROR] {reason}"));
        let mut lines = reason.lines();
        let label = format!("{} failed", self.phase.label());
        display::step_failed(&label, lines.next().unwrap_or(""));
        for line in lines {
            display::detail(line);
        }
        if self.phase.streams_output() {
            for line in self.output.lines() {
                display::detail(&line);
            }
        }
        let previous_running = self.had_release && self.phase.keeps_previous_release();
        display::outro(failure_outro(previous_running));
        display::summary(&as_pairs(&failure_summary(&self.app, branch)));
    }
}

fn intro_title(app: &str, commit: Option<&Commit>) -> String {
    match commit {
        Some(commit) if commit.subject.is_empty() => {
            format!("Deploying {app}  {}", commit.short_sha)
        }
        Some(commit) => format!(
            "Deploying {app}  {} \"{}\"",
            commit.short_sha, commit.subject
        ),
        None => format!("Deploying {app}"),
    }
}

/// `12.4s` under a minute, `2m 03s` from there.
pub(crate) fn format_duration(took: Duration) -> String {
    let secs = took.as_secs_f64();
    if secs < 60.0 {
        return format!("{secs:.1}s");
    }
    let whole = took.as_secs();
    format!("{}m {:02}s", whole / 60, whole % 60)
}

fn failure_outro(previous_running: bool) -> &'static str {
    if previous_running {
        "Deploy failed. The previous release is still running."
    } else {
        "Deploy failed."
    }
}

fn success_summary(app: &str, outcome: &Outcome) -> Vec<(&'static str, String)> {
    let mut rows = vec![("App", app.to_string())];
    if let Some(commit) = outcome.commit {
        rows.push((
            "Commit",
            format!("{}  {}", commit.short_sha, commit.subject),
        ));
    }
    for url in outcome.urls {
        rows.push(("URL", url.clone()));
    }
    if !outcome.workers.is_empty() {
        rows.push(("Workers", outcome.workers.to_string()));
    }
    rows.push(("Logs", format!("rk logs {app}")));
    rows
}

fn failure_summary(app: &str, branch: &str) -> Vec<(&'static str, String)> {
    vec![
        ("Full log", format!("rk logs {app} --deploy")),
        ("Retry", format!("git push riku {branch}")),
    ]
}

fn as_pairs<'a>(rows: &'a [(&'static str, String)]) -> Vec<(&'a str, &'a str)> {
    rows.iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect()
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod tests;
