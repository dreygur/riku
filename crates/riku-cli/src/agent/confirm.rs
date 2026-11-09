//! One-use confirmation tokens for destructive agent commands.
//!
//! Security model: a token is 256 random bits from the OS CSPRNG, so it
//! cannot be guessed. Its record lives in the private agent state directory
//! (`state.rs`: `0700`, files `0600`, opened with `O_NOFOLLOW`), never in a
//! shared directory such as `/tmp`, so another local user can neither plant
//! nor redirect one. A token is checked to be lowercase hex before it becomes
//! part of a path, binds one action on one app, expires after
//! `TOKEN_TTL_SECS`, and is deleted on first use.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use riku_util::token::generate_token;

use super::state::{open_private, Access};
use super::types::AgentResponse;

const TOKEN_TTL_SECS: u64 = 300;
const TOKEN_HEX_LEN: usize = 64;
const RECORD_PREFIX: &str = "confirm-";

/// Why a token was refused. The agent sees one error code either way.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Malformed,
    Unknown,
    Mismatch,
    Expired,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn is_token_shape(token: &str) -> bool {
    token.len() == TOKEN_HEX_LEN
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn record_path(state: &Path, token: &str) -> PathBuf {
    state.join(format!("{RECORD_PREFIX}{token}"))
}

fn record_text(action: &str, app: &str, expires: u64) -> String {
    format!("{action}\n{app}\n{expires}\n")
}

#[allow(clippy::question_mark)]
fn issue_at(state: &Path, action: &str, app: &str, now: u64) -> std::io::Result<String> {
    let token = generate_token();
    let mut file = match open_private(&record_path(state, &token), Access::Rewrite) {
        Ok(file) => file,
        Err(e) => return Err(e),
    };
    let text = record_text(action, app, now + TOKEN_TTL_SECS);
    match file.write_all(text.as_bytes()) {
        Ok(()) => Ok(token),
        Err(e) => Err(e),
    }
}

fn take_record(state: &Path, token: &str) -> Result<String, Refusal> {
    let path = record_path(state, token);
    let mut file = match open_private(&path, Access::Read) {
        Ok(file) => file,
        Err(_) => return Err(Refusal::Unknown),
    };
    let mut text = String::new();
    if file.read_to_string(&mut text).is_err() {
        return Err(Refusal::Unknown);
    }
    if std::fs::remove_file(&path).is_err() {
        return Err(Refusal::Unknown);
    }
    Ok(text)
}

#[allow(clippy::question_mark)]
fn redeem_at(state: &Path, action: &str, app: &str, token: &str, now: u64) -> Result<(), Refusal> {
    if !is_token_shape(token) {
        return Err(Refusal::Malformed);
    }
    let text = match take_record(state, token) {
        Ok(text) => text,
        Err(refusal) => return Err(refusal),
    };

    let mut lines = text.lines();
    if lines.next() != Some(action) || lines.next() != Some(app) {
        return Err(Refusal::Mismatch);
    }
    let expires = match lines.next().map(str::parse::<u64>) {
        Some(Ok(expires)) => expires,
        _ => return Err(Refusal::Unknown),
    };
    if now > expires {
        return Err(Refusal::Expired);
    }
    Ok(())
}

/// Issue a token that allows `action` on `app` once, within `TOKEN_TTL_SECS`.
pub fn issue(state: &Path, action: &str, app: &str) -> std::io::Result<String> {
    issue_at(state, action, app, now_secs())
}

/// Spend `token` for `action` on `app`. The record is deleted whether or not
/// it matches, so a token can never be tried twice.
pub fn redeem(state: &Path, action: &str, app: &str, token: &str) -> Result<(), Refusal> {
    redeem_at(state, action, app, token, now_secs())
}

/// The agent reply asking for confirmation, carrying a fresh token.
pub fn request(state: &Path, action: &str, app: &str) -> AgentResponse {
    match issue(state, action, app) {
        Ok(token) => AgentResponse::confirmation_required(action, app, &token),
        Err(e) => AgentResponse::error(
            "STATE_UNAVAILABLE",
            &format!("Agent state is unavailable: {e}"),
        ),
    }
}

/// `None` when `token` allows `action` on `app`, else the refusal reply.
pub fn refused(state: &Path, action: &str, app: &str, token: &str) -> Option<AgentResponse> {
    match redeem(state, action, app, token) {
        Ok(()) => None,
        Err(_) => Some(AgentResponse::error(
            "INVALID_TOKEN",
            "Invalid or expired confirmation token",
        )),
    }
}

#[cfg(test)]
#[path = "confirm_tests.rs"]
mod tests;
