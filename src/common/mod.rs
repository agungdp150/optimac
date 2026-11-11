//! Shared scanning, categorization, formatting and resource budgets.
pub mod categories;
mod format;
pub mod limits;
mod scan;
pub use format::format_size;
pub use scan::{Report, scan, scan_progress};
