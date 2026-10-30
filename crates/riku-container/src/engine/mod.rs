//! Driver for the Docker compatible container engine CLI.
//!
//! # Security model
//!
//! Every value reaches the engine as a separate argv element, never through a
//! shell. Names, paths and image references that came from a compose file are
//! validated by the types in [`crate::types`] before they are used here.
