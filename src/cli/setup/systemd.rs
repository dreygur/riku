//! Systemd service installation: system-wide (root) and user-level variants.

pub use super::system_service::install_systemd_service;
pub use super::user_service::setup_systemd_service;
