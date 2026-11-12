//! CLI orchestration for plan files only; no recovery or deletion dependency.
use super::{
    Budget,
    config::Config,
    plan_builder,
    plan_store::{PlanStore, StoredPlan},
};
use crate::paths::AppPaths;
use std::{
    ffi::OsString,
    io,
    path::Path,
    sync::atomic::AtomicBool,
    time::{SystemTime, UNIX_EPOCH},
};

pub const WARNING: &str =
    "This plan changes nothing. Applying it is a separate step that does not exist yet.";
fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "Usage: optimac clean plan [--provider ID ...] [--config PATH] [--deep] [--json] | --show PLAN-ID [--json] | --list [--json]",
    )
}
fn totals(plan: &StoredPlan) -> (u64, u64, bool) {
    let mut logical = 0u64;
    let mut allocated = 0u64;
    for t in plan.content.providers.iter().flat_map(|p| &p.targets) {
        logical = logical.saturating_add(t.logical_bytes);
        allocated = allocated.saturating_add(t.allocated_bytes);
    }
    (
        logical,
        allocated,
        plan.content
            .providers
            .iter()
            .any(|p| matches!(p.completeness.as_str(), "partial" | "cancelled")),
    )
}
pub fn json(plan: &StoredPlan) -> serde_json::Value {
    let (logical, allocated, lower) = totals(plan);
    let mut value = serde_json::to_value(plan).expect("serialized plan");
    value["read_only"] = true.into();
    value["warning"] = WARNING.into();
    value["estimated_logical_bytes"] = logical.into();
    value["estimated_allocated_bytes"] = allocated.into();
    value["lower_bound"] = lower.into();
    value
}
pub fn render(plan: &StoredPlan) -> String {
    let (logical, allocated, lower) = totals(plan);
    let mut out = format!(
        "Read-only cleanup plan {}\nEstimated target size: {} logical, {} allocated{}\nEstimated sizes are not guaranteed disk space freed.\n{}\nPlan file saved separately in app data; candidate files are unchanged.\n",
        plan.plan_id,
        crate::format_size(logical),
        crate::format_size(allocated),
        if lower {
            " (lower bound; incomplete providers excluded)"
        } else {
            ""
        },
        WARNING
    );
    let mut rows: Vec<_> = plan.content.providers.iter().collect();
    rows.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then(a.provider.cmp(&b.provider))
    });
    let mut category = "";
    for p in rows {
        if category != p.category {
            category = &p.category;
            out.push_str(&format!("\n{category}\n"));
        }
        let allocated = p
            .targets
            .iter()
            .fold(0u64, |total, t| total.saturating_add(t.allocated_bytes));
        out.push_str(&format!(
            "  {}: {} target(s), {} allocated ({}, {}, {})\n    {}\n",
            p.provider,
            p.targets.len(),
            crate::format_size(allocated),
            p.class,
            p.risk,
            p.completeness,
            p.consequence
        ));
        for t in &p.targets {
            out.push_str(&format!(
                "    {} — {} allocated\n",
                t.path.display(),
                crate::format_size(t.allocated_bytes)
            ));
        }
    }
    out.push_str("\nExcluded\n");
    for p in &plan.content.providers {
        for e in &p.excluded {
            out.push_str(&format!(
                "  {}: {} — {}\n",
                p.provider,
                e.path.display(),
                e.reason
            ));
        }
    }
    out
}
pub fn run(args: &[OsString], paths: &AppPaths, cancel: &AtomicBool) -> io::Result<String> {
    let mut selected = vec![];
    let mut config = None;
    let mut show = None;
    let mut list = false;
    let mut deep = false;
    let mut as_json = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].to_str() {
            Some("--provider") if i + 1 < args.len() => {
                i += 1;
                selected.push(args[i].to_str().ok_or_else(usage)?.to_owned());
            }
            Some("--config") if i + 1 < args.len() && config.is_none() => {
                i += 1;
                config = Some(Path::new(&args[i]));
            }
            Some("--show") if i + 1 < args.len() && show.is_none() => {
                i += 1;
                show = Some(args[i].to_str().ok_or_else(usage)?);
            }
            Some("--list") if !list => list = true,
            Some("--json") if !as_json => as_json = true,
            Some("--deep") if !deep => deep = true,
            _ => return Err(usage()),
        }
        i += 1;
    }
    if (list || show.is_some()) && (!selected.is_empty() || config.is_some() || deep)
        || list && show.is_some()
    {
        return Err(usage());
    }
    if list || show.is_some() {
        let store = match PlanStore::open(paths, &[], false) {
            Err(e) if list && e.kind() == io::ErrorKind::NotFound => {
                return Ok(if as_json {
                    "{\"schema_version\":1,\"plans\":[]}".into()
                } else {
                    "No stored plans.\n".into()
                });
            }
            other => other?,
        };
        if let Some(id) = show {
            let plan = store.load(id)?;
            return if as_json {
                Ok(serde_json::to_string_pretty(&json(&plan))?)
            } else {
                Ok(render(&plan))
            };
        }
        let plans = store.list()?;
        if as_json {
            return Ok(serde_json::to_string_pretty(
                &serde_json::json!({"schema_version":1,"plans":plans.iter().map(json).collect::<Vec<_>>()}),
            )?);
        }
        return Ok(if plans.is_empty() {
            "No stored plans.\n".into()
        } else {
            plans
                .iter()
                .map(|p| {
                    format!(
                        "{} — created {} UTC (Unix seconds), {} providers, {} allocated estimate\n",
                        p.plan_id,
                        p.content.created_at_unix_seconds,
                        p.content.providers.len(),
                        crate::format_size(totals(p).1)
                    )
                })
                .collect()
        });
    }
    selected.sort();
    selected.dedup();
    let config = config.map(Config::load).transpose()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let built = plan_builder::build(
        paths.home(),
        config.as_ref(),
        &selected,
        cancel,
        if deep {
            Budget::deep()
        } else {
            Budget::quick()
        },
        now,
    )?;
    let store = PlanStore::open(paths, &built.scan_roots, true)?;
    match store.load(&built.plan.plan_id) {
        Ok(existing) if existing == built.plan => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => store.save(&built.plan)?,
        Err(e) => return Err(e),
        _ => {
            return Err(io::Error::other(
                "Existing plan differs; refusing overwrite",
            ));
        }
    }
    if as_json {
        Ok(serde_json::to_string_pretty(&json(&built.plan))?)
    } else {
        Ok(render(&built.plan))
    }
}
