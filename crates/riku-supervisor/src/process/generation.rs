//! Versioned deployment generations — the data model backing zero-downtime,
//! health-probed rollouts (see `orchestration.rs`).

/// Lifecycle state of a single deployment generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationStatus {
    /// Currently serving traffic under the canonical process slot.
    Stable,
    /// Actively being polled by the health-probe loop.
    Probing,
    /// Failed its probe window or crashed; rolled back.
    Failed,
}

/// One versioned instance of an app's worker, tracked independently of the
/// canonical `process_id` slot it may or may not currently occupy.
#[derive(Debug, Clone)]
pub struct AppGeneration {
    pub version: u32,
    pub pids: Vec<u32>,
    pub status: GenerationStatus,
    /// The key this generation's process is stored under in
    /// `ProcessManager::processes` while it is not yet the canonical slot.
    pub(super) temp_key: String,
    /// The worker ordinal of the canonical config, restored onto the
    /// process once promoted (the temp key spawns with a shifted ordinal
    /// to avoid colliding with the stable generation's process_id).
    pub(super) canonical_ordinal: u32,
}

/// Result of a background health probe against a generation's `/healthz`-style endpoint.
#[derive(Debug, Clone)]
pub struct ProbeOutcome {
    pub process_id: String,
    pub version: u32,
    pub healthy: bool,
    pub reason: String,
}
