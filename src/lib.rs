//! Shared OptiMac scanning, cleanup and diagnostic functionality.
pub mod actions;
pub mod cleanup;
mod common;
pub mod fixture_paths;
pub mod paths;
pub mod recovery;
pub use common::{Report, categories, format_size, limits, scan, scan_progress};
