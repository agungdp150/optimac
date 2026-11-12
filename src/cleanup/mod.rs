//! Read-only cleanup inventory. These reports never authorize mutations.
use serde::Serialize;
use std::{
    collections::HashSet,
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

mod access;
mod accounting;
pub mod apply_validation;
pub mod category;
pub mod config;
pub mod go;
pub mod pip;
pub mod plan;
pub mod plan_builder;
pub mod plan_command;
pub mod plan_store;
mod projects;
mod providers;
mod registry;
mod report;
mod scan;
#[cfg(test)]
mod tests;
pub mod trust;

use accounting::available;
pub use providers::{
    configured_inventory, configured_inventory_with_progress, inventory, inventory_with_progress,
};
pub use report::{Budget, Inventory, ProviderReport, Status};
use scan::{measure, status, validate};

pub use registry::{Capability, catalog};
