//! The name of one worker copy: `<app>@<kind>.<n>`.
//!
//! It is the supervisor's process id, and the worker config file is that id
//! plus `.toml`, so a config file's name says which process it runs. A
//! container copy's stats row uses the same id with the compose service in
//! place of the kind.

use std::fmt;

use super::{exact_app_name, APP_SEPARATOR};
use crate::procfile::is_valid_kind;

/// Extension of a worker config file.
pub const CONFIG_EXTENSION: &str = "toml";

/// One copy of a Procfile kind of an app.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkerName {
    pub app: String,
    /// Procfile kind, or the compose service of a container copy.
    pub kind: String,
    /// Copy number, counted from 1.
    pub ordinal: u32,
}

impl WorkerName {
    pub fn new(app: &str, kind: &str, ordinal: u32) -> WorkerName {
        WorkerName {
            app: app.to_string(),
            kind: kind.to_string(),
            ordinal,
        }
    }

    /// Reads `<app>@<kind>.<n>` back. `None` for any other shape, including
    /// the legacy one, and for an app name or kind no Procfile app can have.
    #[allow(clippy::question_mark)]
    pub fn parse(id: &str) -> Option<WorkerName> {
        let Some((app, rest)) = id.split_once(APP_SEPARATOR) else {
            return None;
        };
        let Some((kind, ordinal)) = rest.rsplit_once('.') else {
            return None;
        };
        let Some(app) = exact_app_name(app) else {
            return None;
        };
        if !is_valid_kind(kind) || !ordinal.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        match ordinal.parse::<u32>() {
            Ok(ordinal) => Some(WorkerName::new(app, kind, ordinal)),
            Err(_) => None,
        }
    }

    /// `<app>@<kind>.<n>.toml`.
    pub fn file_name(&self) -> String {
        format!("{self}.{CONFIG_EXTENSION}")
    }
}

impl fmt::Display for WorkerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{APP_SEPARATOR}{}.{}",
            self.app, self.kind, self.ordinal
        )
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
