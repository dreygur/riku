//! The `app.restarted` and `app.failed` lifecycle events (Plugin Protocol v1
//! §7.1) the supervisor fires for a crashed process.
//!
//! Each is dispatched on its own thread: `EventBus::emit` runs every
//! subscriber synchronously with its own timeout, and these fire from the
//! supervisor's single threaded monitoring tick, so a slow or unreachable
//! notification target must never delay health checks for other apps.

use crate::plugins::{EventBus, EventName};

/// Where a [`super::ProcessManager`]'s lifecycle events go.
#[derive(Clone, Default)]
pub(crate) struct EventTarget {
    /// The riku tree whose plugins get the events. `None` reads it from the
    /// environment when an event fires, as the running supervisor does.
    pub(crate) paths: Option<riku_config::RikuPaths>,
    /// Extra variables for every subscriber, so a test configures one
    /// without changing the process environment.
    pub(crate) subscriber_env: Vec<(String, String)>,
}

impl EventTarget {
    fn publish(&self, event: EventName, app: String, data: serde_json::Value) {
        let target = self.clone();
        std::thread::spawn(move || {
            let paths = match target.paths {
                Some(paths) => paths,
                None => match riku_config::RikuPaths::from_env() {
                    Ok(paths) => paths,
                    Err(e) => {
                        tracing::error!("Cannot emit {event:?} for {app}: {e}");
                        return;
                    }
                },
            };
            EventBus::new(&paths)
                .with_subscriber_env(target.subscriber_env)
                .publish(event, &app, data);
        });
    }

    /// `app.restarted` for a crash the supervisor just recovered from.
    pub(super) fn app_restarted(
        &self,
        app: String,
        instance: String,
        exit_code: Option<i32>,
        restart_count: u32,
    ) {
        let data = serde_json::json!({
            "instance": instance,
            "exit_code": exit_code,
            "restart_count": restart_count,
        });
        self.publish(EventName::AppRestarted, app, data);
    }

    /// `app.failed` for a process that hit `max_restarts` and is removed
    /// from supervision, the case `app_restarted` never covers.
    pub(super) fn app_failed(
        &self,
        app: String,
        instance: String,
        exit_code: Option<i32>,
        restart_count: u32,
        max_restarts: u32,
    ) {
        let data = serde_json::json!({
            "instance": instance,
            "exit_code": exit_code,
            "restart_count": restart_count,
            "max_restarts": max_restarts,
        });
        self.publish(EventName::AppFailed, app, data);
    }
}
