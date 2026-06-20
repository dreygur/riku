//! Per-SSH-key CLI authorization gate.
//!
//! Every `riku` invocation — whether typed locally or dispatched through the
//! SSH forced command in `authorized_keys` — funnels through `authorize()`
//! before any command runs. It resolves the calling key's access scope and
//! app allowlist (`cli::agent::auth::resolve_key_scope`/`resolve_key_apps`,
//! populated from the `RIKU_AGENT_SCOPE`/`RIKU_KEY_APPS` env vars a scoped
//! SSH key's forced command injects) and checks the requested command
//! against them. Keys with no scope recorded — legacy keys and local admin
//! usage — resolve to `AgentScope::Full` and bypass the gate entirely.

use anyhow::{bail, Result};

use super::agent::auth::{
    check_rate_limit, get_agent_identity, log_agent_action, resolve_key_apps, resolve_key_scope,
};
use super::agent::types::AgentScope;
use super::cmds::{AppsCmd, ConfigCmd, StatsCmd};
use super::Commands;

/// Authorize `command` against the calling key's scope and app allowlist.
/// Returns `Ok(())` if allowed, or an error explaining the denial.
pub fn authorize(command: &Commands) -> Result<()> {
    let scope = resolve_key_scope();
    if scope == AgentScope::Full {
        return Ok(());
    }

    let (action, app) = classify(command);
    let identity = get_agent_identity().unwrap_or_else(|| "unknown-key".to_string());

    let allowed = scope.allows(action) && app_permitted(&scope, app);

    log_agent_action(&identity, action, app.unwrap_or("-"), allowed);

    if !allowed {
        bail!(
            "permission denied: key scope '{:?}' may not run '{}'{}",
            scope,
            action,
            app.map(|a| format!(" for app '{}'", a)).unwrap_or_default()
        );
    }

    if !check_rate_limit(&identity, &scope) {
        bail!("rate limit exceeded for this key's scope; try again shortly");
    }

    Ok(())
}

/// Whether `app` is within the key's allowlist. A scoped (non-Full) key with
/// no allowlist recorded is granted no apps. `app == None` means the action
/// would operate across all apps (e.g. `riku ps` with no app given) — only
/// `Full` keys (already short-circuited above) may do that.
fn app_permitted(_scope: &AgentScope, app: Option<&str>) -> bool {
    match app {
        Some(app) => resolve_key_apps()
            .map(|apps| apps.iter().any(|a| a == app))
            .unwrap_or(false),
        None => false,
    }
}

/// Map a parsed `Commands` to `(action, app)` for the scope/allowlist check.
/// Action names match `AgentScope::allows()`'s table in `cli::agent::types`.
fn classify(command: &Commands) -> (&'static str, Option<&str>) {
    match command {
        Commands::Apps { cmd: None } => ("apps", None),
        Commands::Apps {
            cmd: Some(AppsCmd::Create { name }),
        } => ("deploy", Some(name)),
        Commands::Apps {
            cmd: Some(AppsCmd::Info { name }),
        } => ("apps", Some(name)),
        Commands::Apps {
            cmd: Some(AppsCmd::Destroy { name }),
        } => ("destroy", Some(name)),

        // The `riku agent` subcommand has its own dedicated scope/rate-limit
        // enforcement (cli::agent::execute) — don't double-gate it here.
        Commands::Agent { .. } => ("agent", None),

        Commands::Config(ConfigCmd::Show { app }) => ("config:show", Some(app)),
        Commands::Config(ConfigCmd::Live { app }) => ("config:show", Some(app)),
        Commands::Config(ConfigCmd::Get { app, .. }) => ("config:get", Some(app)),
        Commands::Config(ConfigCmd::Set { app, .. }) => ("config:set", Some(app)),
        Commands::Config(ConfigCmd::Unset { app, .. }) => ("config:unset", Some(app)),

        // Container export/remote-deploy isn't tied to a single app's
        // allowlist semantics and moves whole build contexts — admin-only.
        Commands::Container { .. } => ("container", None),

        Commands::Deploy { app, .. } => ("deploy", Some(app)),
        Commands::Destroy { app } => ("destroy", Some(app)),
        Commands::Logs { app, .. } => ("logs", Some(app)),
        Commands::Ps { app, .. } => ("ps", app.as_deref()),
        Commands::Stats(StatsCmd::All) => ("stats", None),
        Commands::Stats(StatsCmd::App { app }) => ("stats", Some(app)),
        Commands::Run { app, .. } => ("run", Some(app)),
        Commands::Restart { app, .. } => ("restart", Some(app)),
        Commands::Stop { app } => ("stop", Some(app)),

        Commands::Init { .. } => ("init", None),
        Commands::Update => ("update", None),
        Commands::InstallPlugins { .. } => ("install-plugins", None),
        Commands::Supervisor => ("supervisor", None),
        Commands::Plugin(_) => ("plugin", None),
        Commands::Hook(_) => ("hook", None),

        // git push/pull are the normal app-scoped deploy flow for a scoped
        // key — both `git-receive-pack` and the post-receive hook it spawns
        // inherit RIKU_AGENT_SCOPE/RIKU_KEY_APPS from the SSH session, so
        // these must stay app-scoped ("deploy"), not admin-only, or a
        // legitimately scoped key could never git push.
        Commands::GitReceivePack { app } => ("deploy", Some(app)),
        Commands::GitHook { app, .. } => ("deploy", Some(app)),
        // git-upload-pack (clone/fetch) is read-only; reuse the
        // always-allowed "apps" tier, still gated by the app allowlist.
        Commands::GitUploadPack { app } => ("apps", Some(app)),

        // Rooted at git_root with no per-app boundary — admin-only.
        Commands::Scp { .. } => ("scp", None),
        // Internal-only: exec'd by the supervisor (NsShim) or invoked
        // locally for diagnostics (DumpState); never reached over SSH with
        // a scoped key's env vars in practice, but admin-only by default.
        Commands::NsShim => ("ns-shim", None),
        Commands::DumpState => ("dump-state", None),

        // Provisioning/restricting SSH keys is itself a privilege-granting
        // action — a scoped key must never be able to widen its own or
        // another key's access. Admin-only.
        Commands::Setup(_) => ("setup", None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize tests that mutate process-global env vars.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_scope<F: FnOnce()>(scope: Option<&str>, apps: Option<&str>, f: F) {
        let _guard = ENV_LOCK.lock().unwrap();
        match scope {
            Some(s) => std::env::set_var("RIKU_AGENT_SCOPE", s),
            None => std::env::remove_var("RIKU_AGENT_SCOPE"),
        }
        match apps {
            Some(a) => std::env::set_var("RIKU_KEY_APPS", a),
            None => std::env::remove_var("RIKU_KEY_APPS"),
        }
        f();
        std::env::remove_var("RIKU_AGENT_SCOPE");
        std::env::remove_var("RIKU_KEY_APPS");
    }

    #[test]
    fn no_scope_recorded_allows_everything() {
        with_scope(None, None, || {
            assert!(authorize(&Commands::Init { no_systemd: false }).is_ok());
            assert!(authorize(&Commands::Destroy {
                app: "anything".into()
            })
            .is_ok());
        });
    }

    #[test]
    fn readonly_scope_with_app_allowlist() {
        with_scope(Some("readonly"), Some("demoapp"), || {
            assert!(authorize(&Commands::Logs {
                app: "demoapp".into(),
                process: "*".into(),
                deploy: false,
                follow: false,
            })
            .is_ok());

            assert!(authorize(&Commands::Destroy {
                app: "demoapp".into()
            })
            .is_err());

            assert!(authorize(&Commands::Logs {
                app: "otherapp".into(),
                process: "*".into(),
                deploy: false,
                follow: false,
            })
            .is_err());

            assert!(authorize(&Commands::Init { no_systemd: false }).is_err());
        });
    }

    #[test]
    fn staging_scope_allows_deploy_within_allowlist() {
        with_scope(Some("staging"), Some("demoapp"), || {
            assert!(authorize(&Commands::Deploy {
                app: "demoapp".into(),
                from: None,
            })
            .is_ok());

            assert!(authorize(&Commands::Deploy {
                app: "otherapp".into(),
                from: None,
            })
            .is_err());
        });
    }

    #[test]
    fn scoped_key_with_no_allowlist_grants_no_apps() {
        with_scope(Some("production"), None, || {
            assert!(authorize(&Commands::Logs {
                app: "demoapp".into(),
                process: "*".into(),
                deploy: false,
                follow: false,
            })
            .is_err());
        });
    }
}
