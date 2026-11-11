//! Shared resource budgets for local scans and external commands.
use std::time::Duration;
pub const SCAN_MAX_ENTRIES: u64 = 100_000;
pub const SCAN_MAX_DEPTH: usize = 32;
pub const SCAN_TIMEOUT: Duration = Duration::from_secs(30);
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
pub const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(50);
pub const COMMAND_OUTPUT_LIMIT: usize = 256 * 1024;
pub const CLEANUP_MAX_ENTRIES: usize = 10_000;
pub const CLEANUP_MAX_DEPTH: usize = 16;
pub const CLEANUP_TIMEOUT: Duration = Duration::from_secs(15);
pub const CACHE_MIN_AGE_SECONDS: i64 = 30 * 24 * 60 * 60;
