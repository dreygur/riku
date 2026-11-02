//! Rolling replacement of container copies.

// Project rule 16 forbids `?`, and this lint asks for exactly that rewrite.
#![allow(clippy::question_mark)]

pub mod health;
#[cfg(test)]
mod mock_copies;
mod probe;
pub mod replace;
pub mod scale;

pub use replace::{replace_all, CopyHandle, CopyRuntime, LiveCopy, NoRoutes, RouteSink};
pub use scale::scale_to;
