//! `riku certs`: an app serving a certificate its owner supplied instead of
//! one riku issues.
//!
//! Security model: stdin is untrusted and bounded. The bundle is validated
//! before anything is written (chain parses, key matches the leaf, not
//! expired), the key is written `0600`, and neither the key nor any PEM text
//! is ever printed. The app name is validated and must name an existing app.

#![allow(clippy::question_mark)]

use std::io::{IsTerminal, Read};

use anyhow::{anyhow, bail, Result};
use chrono::DateTime;

use super::certs_reapply::{reapply_route, reconfigure};
use super::settings_cmd::known_app;
use crate::config::RikuPaths;
use crate::own_cert::bundle::validate;
use crate::own_cert::info::{self, CertInfo, Expiry};
use crate::own_cert::store::{self, Source};
use crate::util::display;

const MAX_INPUT_BYTES: u64 = 256 * 1024;

/// What `set` found out about the certificate it installed.
#[derive(Debug)]
struct SetOutcome {
    info: CertInfo,
    /// RIKU_DOMAINS entries the certificate does not cover.
    uncovered: Vec<String>,
}

fn read_stdin() -> Result<String> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        bail!("pipe the certificate chain and key in on stdin: cat fullchain.pem key.pem | riku certs set <app>");
    }
    let mut text = String::new();
    if let Err(e) = stdin
        .lock()
        .take(MAX_INPUT_BYTES + 1)
        .read_to_string(&mut text)
    {
        return Err(anyhow!("cannot read stdin: {e}"));
    }
    if text.len() as u64 > MAX_INPUT_BYTES {
        bail!("the input is larger than {} KiB", MAX_INPUT_BYTES / 1024);
    }
    Ok(text)
}

fn app_domains(paths: &RikuPaths, app: &str) -> Vec<String> {
    let env = match crate::config::app_config::load_env(paths, app) {
        Ok(loaded) => loaded.env,
        Err(_) => return Vec::new(),
    };
    let Some(list) = env.get("RIKU_DOMAINS") else {
        return Vec::new();
    };
    list.split(',')
        .map(str::trim)
        .filter(|domain| !domain.is_empty())
        .map(str::to_string)
        .collect()
}

/// Validate `input`, save it as `app`'s certificate and put it live with
/// `reapply`. Nothing is written when validation fails.
fn install_bundle(
    paths: &RikuPaths,
    app: &str,
    input: &str,
    reapply: impl Fn(&str) -> Result<()>,
) -> Result<SetOutcome> {
    let bundle = match validate(input) {
        Ok(bundle) => bundle,
        Err(e) => return Err(e),
    };
    let leaf = crate::own_cert::bundle::first_certificate(&bundle.cert_pem);
    let found = match info::read(leaf) {
        Ok(found) => found,
        Err(e) => return Err(e),
    };
    if info::classify(found.expires_at, info::now()) == Expiry::Expired {
        bail!("the certificate has expired");
    }
    let domains = app_domains(paths, app);
    // acme.sh renews into the same files; without this a renewal would put
    // a Let's Encrypt certificate back over the user's pair.
    crate::nginx::forget_acme(app, &domains, paths);
    if let Err(e) = store::install(paths, app, &bundle) {
        return Err(e);
    }
    let uncovered = domains
        .into_iter()
        .filter(|domain| !info::covers(&found.names, domain))
        .collect();
    match reapply(app) {
        Ok(()) => Ok(SetOutcome {
            info: found,
            uncovered,
        }),
        Err(e) => Err(anyhow!(
            "the certificate is saved but the route was not updated: {e:#}; the next deploy applies it"
        )),
    }
}

fn source_label(source: &Source) -> &'static str {
    match source {
        Source::Own => "own certificate",
        Source::LetsEncrypt => "Let's Encrypt",
        Source::SelfSigned => "self-signed",
        Source::None => "none",
    }
}

fn expires_line(expires_at: i64) -> String {
    let date = DateTime::from_timestamp(expires_at, 0)
        .map(|at| at.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    match info::classify(expires_at, info::now()) {
        Expiry::Expired => format!("expires: {date} (expired)"),
        Expiry::Soon { days } | Expiry::Valid { days } => {
            format!("expires: {date} (in {days} days)")
        }
    }
}

fn show_lines(paths: &RikuPaths, app: &str) -> Result<Vec<String>> {
    let app = match known_app(paths, app) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    let source = store::source(paths, &app);
    let mut lines = vec![format!("source: {}", source_label(&source))];
    let Some(leaf) = store::leaf_pem(paths, &app) else {
        return Ok(lines);
    };
    let found = match info::read(&leaf) {
        Ok(found) => found,
        Err(e) => return Err(e),
    };
    lines.push(format!("names: {}", found.names.join(", ")));
    lines.push(expires_line(found.expires_at));
    Ok(lines)
}

/// Remove `app`'s own pair and marker, then apply the route through
/// `reapply`, so the router stops serving the removed certificate now and
/// riku issues one as it would for any app.
fn remove_own(paths: &RikuPaths, app: &str, reapply: impl Fn(&str) -> Result<()>) -> Result<()> {
    if store::source(paths, app) != Source::Own {
        bail!("'{app}' has no own certificate, so there is nothing to remove");
    }
    if let Err(e) = store::remove(paths, app) {
        return Err(e);
    }
    reapply(app)
}

/// Read one PEM bundle (chain and key) from stdin and serve it for `app`.
pub fn cmd_certs_set(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = match known_app(paths, app) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    let input = match read_stdin() {
        Ok(input) => input,
        Err(e) => return Err(e),
    };
    let outcome = match install_bundle(paths, &app, &input, |name| reapply_route(paths, name)) {
        Ok(outcome) => outcome,
        Err(e) => return Err(e),
    };
    for domain in &outcome.uncovered {
        display::warn(&format!("the certificate does not cover {domain}"));
    }
    display::step_done(&format!("'{app}' now serves your certificate"), "");
    display::detail(&expires_line(outcome.info.expires_at));
    Ok(())
}

/// Print where `app`'s certificate came from, the names it covers and its
/// end date.
pub fn cmd_certs_show(paths: &RikuPaths, app: &str) -> Result<()> {
    match show_lines(paths, app) {
        Ok(lines) => {
            lines.iter().for_each(|line| display::detail(line));
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Remove `app`'s own certificate and apply the route again, which issues
/// a certificate the way a deploy does.
pub fn cmd_certs_unset(paths: &RikuPaths, app: &str) -> Result<()> {
    let app = match known_app(paths, app) {
        Ok(app) => app,
        Err(e) => return Err(e),
    };
    if let Err(e) = remove_own(paths, &app, |name| reconfigure(paths, name)) {
        return Err(e);
    }
    display::step_done(
        &format!("removed the own certificate of '{app}'; the route now uses one riku issues"),
        "",
    );
    Ok(())
}

#[cfg(test)]
#[path = "certs_cmd_tests.rs"]
mod tests;
