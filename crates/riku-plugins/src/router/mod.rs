//! Router seam — exposes apps to the network (`PLUGIN_PROTOCOL.md` §6.2).
//!
//! A **singleton** seam: exactly one router is active per host, chosen by the
//! `RIKU_ROUTER` config (default `nginx`, the built-in generator). This module
//! is the raw verb dispatch to a router *plugin*; the nginx-vs-plugin decision
//! and the per-app request shaping live in the deploy-layer orchestrator
//! the deploy-layer router orchestrator.

mod dispatch;

pub use dispatch::run_verb;
