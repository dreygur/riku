//! Skeleton generator for `riku plugins scaffold` (ROADMAP E0).
//!
//! Produces a working bundle directory for one of the implemented seam types,
//! so a new author starts from something that already parses and runs.

use anyhow::{bail, Result};

/// The implemented seam types a skeleton can target.
pub enum SeamType {
    Runtime,
    Addon,
    Notifier,
}

impl SeamType {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "runtime" => Ok(SeamType::Runtime),
            "addon" => Ok(SeamType::Addon),
            "notifier" => Ok(SeamType::Notifier),
            other => bail!("unknown plugin type '{other}' (supported: runtime, addon, notifier)"),
        }
    }

    fn type_str(&self) -> &'static str {
        match self {
            SeamType::Runtime => "runtime",
            SeamType::Addon => "addon",
            SeamType::Notifier => "notifier",
        }
    }

    /// `riku-plugin.toml` contents for a plugin named `name`.
    pub fn manifest(&self, name: &str) -> String {
        let common = format!(
            "name        = \"{name}\"\nversion     = \"0.1.0\"\ntype        = \"{}\"\napi         = 1\nentry       = \"bin/{name}\"\ndescription = \"TODO: describe {name}\"\nauthor      = \"TODO\"\n",
            self.type_str()
        );
        match self {
            SeamType::Runtime => common,
            SeamType::Addon => format!("{common}\n[capabilities]\nwrites = [\"data_dir\"]\n"),
            SeamType::Notifier => format!(
                "{common}\n[capabilities]\nnetwork = true\n\n[events]\nsubscribe = [\"deploy.finished\", \"deploy.failed\"]\nmode      = \"observe\"\n"
            ),
        }
    }

    /// The `bin/<name>` skeleton script.
    pub fn entry_script(&self, name: &str) -> String {
        match self {
            SeamType::Runtime => runtime_script(name),
            SeamType::Addon => addon_script(name),
            SeamType::Notifier => notifier_script(name),
        }
    }
}

fn runtime_script(name: &str) -> String {
    format!(
        r#"#!/bin/sh
# {name} — Riku runtime plugin (Plugin Protocol v1).
# Verbs: detect | build | env | start. Context in RIKU_APP, RIKU_APP_PATH, …
set -eu

verb="${{1:-}}"
case "$verb" in
  detect)
    # exit 0 if this runtime handles the app, else exit 1.
    [ -f "$RIKU_APP_PATH/TODO-marker" ] && exit 0
    exit 1
    ;;
  build)
    cd "$RIKU_APP_PATH"
    echo "TODO: install dependencies" >&2
    ;;
  env)
    # Print KEY=VALUE lines to inject into the app environment.
    echo "EXAMPLE=1"
    ;;
  start)
    # Print the command the supervisor should run.
    echo "TODO-start-command"
    ;;
  *) echo "{name}: unknown verb '$verb'" >&2; exit 1 ;;
esac
"#
    )
}

fn addon_script(name: &str) -> String {
    format!(
        r#"#!/bin/sh
# {name} — Riku addon (Plugin Protocol v1).
# Verbs receive a JSON request on stdin and emit a JSON response on stdout.
# Context: RIKU_ADDON_INSTANCE, RIKU_ADDON_DATA_PATH, RIKU_APP (bind/unbind).
set -eu

verb="${{1:-}}"
request="$(cat)"   # JSON request (unused in this skeleton)

case "$verb" in
  provision)
    mkdir -p "$RIKU_ADDON_DATA_PATH"
    echo '{{}}'
    ;;
  bind)
    # Return env to inject into the bound app.
    printf '{{"env":{{"EXAMPLE_URL":"example://%s"}}}}' "$RIKU_ADDON_INSTANCE"
    ;;
  unbind|deprovision)
    echo '{{}}'
    ;;
  backup)
    printf '{{"artifact":"%s/backup"}}' "$RIKU_ADDON_DATA_PATH"
    ;;
  *) echo "{name}: unknown verb '$verb'" >&2; exit 1 ;;
esac
"#
    )
}

fn notifier_script(name: &str) -> String {
    format!(
        r#"#!/bin/sh
# {name} — Riku notifier (Plugin Protocol v1 event subscriber).
# Invoked as `on_event` with one event JSON line on stdin per subscribed event.
set -eu

event_json="$(cat)"
echo "{name}: received $event_json" >&2
# TODO: act on the event (post a webhook, run a migration, …).
"#
    )
}
