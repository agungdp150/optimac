//! Shared OptiMac scanning, cleanup and diagnostic functionality.
pub mod actions;
mod common;
pub mod recovery;
pub use common::{Report, categories, format_size, limits, scan, scan_progress};
