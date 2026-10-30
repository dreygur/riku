//! Router seam: exposes apps to the network (`PLUGIN_PROTOCOL.md` §6.2).
//!
//! A **singleton** seam: exactly one router is active per host, named in
//! `<riku root>/ROUTER` (default `nginx`). nginx and Caddy are built in; this
//! module is the raw verb dispatch to a third party router *plugin*. The
//! choice between them and the per-app request shaping live in the
//! deploy-layer router orchestrator.

mod dispatch;

pub use dispatch::run_verb;
