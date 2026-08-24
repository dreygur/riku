//! Repository-layer read access to the supervisor's `stats.json` file.
//!
//! `StatsManager::write_stats_to_file` (in `manager.rs`) is the writer; this
//! is the matching typed reader for the CLI side, which only ever consumes
//! the file, never the in-memory `StatsManager`.

use std::fs;
use std::path::Path;

use super::types::AppStats;

/// Read and parse `stats.json` into typed `AppStats`.
///
/// Returns `None` if the file doesn't exist, can't be read, or fails to
/// parse (an older/newer supervisor version wrote an incompatible shape),
/// callers treat that the same as "supervisor isn't running" and fall back
/// to a worker-config-only view instead of hard-failing.
pub fn read_stats_file(path: &Path) -> Option<Vec<AppStats>> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}
