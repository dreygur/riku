//! Which router `riku init` sets up: the one the operator asked for, the one
//! already stored, or the one installed on the host.
//!
//! The decision is a pure function so every case is tested without touching
//! the host. Only the prompt reads stdin, and only from a terminal.

use std::io::{self, BufRead, IsTerminal, Write};

use anyhow::{anyhow, Result};

use crate::config::program::installed_program;
use crate::config::router::Router;

const BOTH_INSTALLED: &str = "nginx and Caddy are both installed; run init again with \
     RIKU_ROUTER=nginx or RIKU_ROUTER=caddy";
const NONE_INSTALLED: &str = "no router found: install nginx (apt install nginx) or Caddy, \
     then run riku init again";

/// What init does about the router.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RouterChoice {
    Use(Router),
    /// Ask the operator to pick one of these.
    Ask(Vec<Router>),
    Fail(String),
}

/// The built in routers whose program is installed on this host.
pub(super) fn installed_routers() -> Vec<Router> {
    [("nginx", Router::Nginx), ("caddy", Router::Caddy)]
        .into_iter()
        .filter(|(program, _)| installed_program(program).is_some())
        .map(|(_, router)| router)
        .collect()
}

/// The router `RIKU_ROUTER` names, if set.
pub(super) fn requested_router() -> Option<Router> {
    match std::env::var("RIKU_ROUTER") {
        Ok(raw) if !raw.trim().is_empty() => Some(Router::from(raw.as_str())),
        _ => None,
    }
}

/// Decide, in order: `requested` (`RIKU_ROUTER`, any name); `stored`; nginx
/// when the riku tree already existed (`!fresh`), the old default; the only
/// installed router; ask when two are installed and init has a terminal;
/// otherwise fail with what to do.
pub(super) fn choose_router(
    installed: &[Router],
    requested: Option<Router>,
    stored: Option<Router>,
    fresh: bool,
    interactive: bool,
) -> RouterChoice {
    if let Some(router) = requested.or(stored) {
        return RouterChoice::Use(router);
    }
    if !fresh {
        return RouterChoice::Use(Router::Nginx);
    }
    match installed {
        [] => RouterChoice::Fail(NONE_INSTALLED.to_string()),
        [only] => RouterChoice::Use(only.clone()),
        several if interactive => RouterChoice::Ask(several.to_vec()),
        _ => RouterChoice::Fail(BOTH_INSTALLED.to_string()),
    }
}

/// The offered router an answer names, ignoring spaces and case.
fn router_from_answer(answer: &str, offered: &[Router]) -> Option<Router> {
    let wanted = answer.trim().to_ascii_lowercase();
    offered
        .iter()
        .find(|router| router.name() == wanted)
        .cloned()
}

/// Turn a choice into a router, prompting for `Ask`. An answer that names
/// none of the offered routers fails like a missing terminal does.
pub(super) fn resolve(choice: RouterChoice) -> Result<Router> {
    let offered = match choice {
        RouterChoice::Use(router) => return Ok(router),
        RouterChoice::Fail(message) => return Err(anyhow!(message)),
        RouterChoice::Ask(offered) => offered,
    };
    let names: Vec<&str> = offered.iter().map(Router::name).collect();
    print!("Router to use [{}]: ", names.join("/"));
    let _ = io::stdout().flush();
    let mut answer = String::new();
    if io::stdin().lock().read_line(&mut answer).is_err() {
        return Err(anyhow!(BOTH_INSTALLED));
    }
    match router_from_answer(&answer, &offered) {
        Some(router) => Ok(router),
        None => Err(anyhow!(BOTH_INSTALLED)),
    }
}

/// Whether init can ask: stdin is a terminal.
pub(super) fn interactive() -> bool {
    io::stdin().is_terminal()
}

#[cfg(test)]
#[path = "router_choice_tests.rs"]
mod tests;
