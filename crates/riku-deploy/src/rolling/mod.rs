//! Rolling replacement of container copies.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

pub mod health;
pub mod replace;

pub use replace::{replace_all, CopyHandle, CopyRuntime, LiveCopy, NoRoutes, RouteSink};
