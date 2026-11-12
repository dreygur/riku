//! What a deploy prints, on the rail from `riku_util::display`: an intro
//! naming the app and commit, one step per phase, then an outro and a
//! summary. A failed deploy names the phase that failed and says how to
//! retry. Everything goes to stdout, the build output streamed by
//! `tee_output` included, so an ssh client keeps the order.
//!
//! Every line also lands in the deploy log through [`DeployLogger`].

use std::time::{Duration, Instant};

use crate::config::RikuPaths;
use crate::git_ops::Commit;
use crate::releases::ReleaseLog;
use crate::util::deploy_logger::DeployLogger;
use crate::util::display::{self, render_line, Mark, OutputTail, Stream};

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
    /// Set once the deploy starts replacing what runs (worker configs or
    /// containers). From then on the previous release no longer runs as it
    /// was, and what replaces it runs the new checkout, so a failure keeps
    /// the checkout where it is.
    release_replaced: bool,
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
            release_replaced: false,
        }
    }

    /// Record that worker configs or containers are about to change, so a
    /// later failure neither resets the checkout under them nor claims the
    /// old release still runs.
    pub(crate) fn release_replacing(&mut self) {
        self.release_replaced = true;
    }

    /// Whether the deploy started replacing what runs.
    pub(crate) fn release_replaced(&self) -> bool {
        self.release_replaced
    }

    /// Whether the app still runs its previous release unchanged after a
    /// failure at this point.
    pub(crate) fn previous_release_runs(&self) -> bool {
        self.had_release && !self.release_replaced
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

    /// The failed phase with `err`, the outro and the summary that says how
    /// to retry, all on stdout like the rest of the rail.
    pub(crate) fn failed(&mut self, err: &anyhow::Error, branch: &str) {
        let reason = format!("{err:#}");
        display::mark_failure_shown(&reason);
        self.log.log_raw(&format!("[ERROR] {reason}"));
        let label = format!("{} failed", self.phase.label());
        let block = render_failure(
            &label,
            &reason,
            self.previous_release_runs(),
            Stream::Stdout.colour(),
        );
        display::emit(Stream::Stdout, &block);
        display::summary(&as_pairs(&failure_summary(&self.app, branch)));
    }
}

/// The `■` line of the failed step, the rest of its reason on `│` lines,
/// and the outro. One block on one stream: `■` on stderr reached an ssh
/// client after the outro on stdout. The build output is not repeated
/// here, since every line of it was already streamed on the rail.
fn render_failure(label: &str, reason: &str, previous_kept: bool, colour: bool) -> String {
    let mut lines = without_label(reason, label).lines();
    let mut block = render_line(Mark::Error, label, lines.next().unwrap_or(""), colour);
    for line in lines {
        block.push('\n');
        block.push_str(&render_line(Mark::Rail, line, "", colour));
    }
    block.push('\n');
    block.push_str(&render_line(Mark::Rail, "", "", colour));
    block.push('\n');
    block.push_str(&render_line(
        Mark::Outro,
        failure_outro(previous_kept),
        "",
        colour,
    ));
    block
}

/// `reason` without a leading `<label>: `, so `Build failed: plugin ..`
/// does not read `Build failed  Build failed: plugin ..` on the rail.
fn without_label<'a>(reason: &'a str, label: &str) -> &'a str {
    match reason.get(..label.len()) {
        Some(head) if head.eq_ignore_ascii_case(label) => {
            reason[label.len()..].strip_prefix(": ").unwrap_or(reason)
        }
        _ => reason,
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

fn failure_outro(previous_kept: bool) -> &'static str {
    if previous_kept {
        "Deploy failed. The previous release was left as it was."
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
