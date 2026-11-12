//! Read-only Slice 2 preflight. Passing this check is NOT mutation authority.
//! A future apply path must repeat checks at its existing recovery move boundary.
use super::{
    Budget, ProviderReport, Status,
    config::Config,
    plan_store::{Provider, StoredPlan, Target},
};
use std::{
    collections::HashSet,
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

#[derive(Debug)]
pub struct UnitCheck {
    pub provider: String,
    pub path: PathBuf,
    pub refusal: Option<String>,
}

/// Checks schema/hash and age before reading any target path. Future timestamps
/// fail closed rather than extending a plan's lifetime following clock changes.
pub fn fresh_plan(plan: &StoredPlan, now: u64) -> io::Result<()> {
    plan.validate()?;
    let age = now
        .checked_sub(plan.content.created_at_unix_seconds)
        .ok_or_else(|| io::Error::other("Plan creation time is in the future"))?;
    if age > 600 {
        return Err(io::Error::other(
            "Plan is older than 10 minutes; create a fresh plan",
        ));
    }
    Ok(())
}

fn provider_config(home: &Path, provider: &Provider) -> io::Result<Config> {
    if !super::plan_builder::ALLOWED.contains(&provider.provider.as_str()) {
        return Err(io::Error::other("Provider outside the v1 allow-list"));
    }
    let catalog = super::catalog();
    let policy = catalog
        .iter()
        .find(|p| p.provider == provider.provider)
        .ok_or_else(|| io::Error::other("Unknown provider"))?;
    if policy.class != "rebuildable_cache"
        || policy.risk == "protected"
        || provider.class != policy.class
        || provider.risk != policy.risk
    {
        return Err(io::Error::other("Provider classification is not eligible"));
    }
    if !provider.root.starts_with(home)
        || provider.root == home
        || provider.root.starts_with(home.join("OrbStack"))
        || super::providers::protected_path(&provider.root)
    {
        return Err(io::Error::other(
            "Provider root outside approved home scope",
        ));
    }
    let root = provider
        .root
        .to_str()
        .ok_or_else(|| io::Error::other("Non-UTF8 root"))?;
    // Reuse the registry's current configured-root validation; stored JSON cannot
    // nominate an unrelated user directory by claiming an allowed provider ID.
    let rule = toml::Table::from_iter([
        ("id".into(), toml::Value::String(provider.provider.clone())),
        ("enabled".into(), toml::Value::Boolean(true)),
        ("operation".into(), toml::Value::String("inventory".into())),
        (
            "scan_roots".into(),
            toml::Value::Array(vec![toml::Value::String(root.into())]),
        ),
    ]);
    let config = Config::parse(
        &toml::to_string(&toml::Table::from_iter([
            ("schema_version".into(), toml::Value::Integer(2)),
            (
                "rule".into(),
                toml::Value::Array(vec![toml::Value::Table(rule)]),
            ),
        ]))
        .map_err(io::Error::other)?,
    )?;
    let jobs = config.jobs(home, None, None)?;
    if jobs.len() != 1 || jobs[0].unsupported.is_some() {
        return Err(io::Error::other("Provider root discovery unsupported"));
    }
    super::scan::validate(&provider.root)?;
    let home_device = fs::symlink_metadata(home)?.dev();
    let mut current = home.to_path_buf();
    for part in provider
        .root
        .strip_prefix(home)
        .map_err(io::Error::other)?
        .components()
    {
        current.push(part);
        if fs::symlink_metadata(&current)?.dev() != home_device {
            return Err(io::Error::other(
                "Provider ancestor crosses a device boundary",
            ));
        }
    }
    Ok(config)
}

/// Rescans current provider roots with the existing inventory engine. Any
/// incomplete/blocked provider rejects the preflight; individual changed units
/// are reported as skips. This never calls recovery or an external tool.
pub fn preflight(
    home: &Path,
    plan: &StoredPlan,
    now: u64,
    cancel: &AtomicBool,
    budget: Budget,
) -> io::Result<Vec<UnitCheck>> {
    fresh_plan(plan, now)?;
    let mut results = Vec::new();
    for provider in &plan.content.providers {
        if provider.targets.is_empty() {
            continue;
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::other("Revalidation cancelled"));
        }
        let config = provider_config(home, provider)?;
        let report = super::configured_inventory(home, cancel, budget, &config)?;
        if report.providers.len() != 1
            || report.providers[0].status != Status::Complete
            || report.providers[0].root != provider.root
        {
            return Err(io::Error::other(format!(
                "Provider {} is not complete: {}",
                provider.provider,
                report
                    .providers
                    .iter()
                    .map(|r| format!("{:?}: {}", r.status, r.note))
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
        for target in &provider.targets {
            let refusal = revalidate_unit(
                home,
                provider,
                target,
                plan.content.created_at_unix_seconds,
                cancel,
                budget,
            )
            .err()
            .map(|e| e.to_string());
            results.push(UnitCheck {
                provider: provider.provider.clone(),
                path: target.path.clone(),
                refusal,
            });
        }
    }
    Ok(results)
}

fn identity_matches(target: &Target, meta: &fs::Metadata) -> bool {
    !meta.is_symlink()
        && ((target.kind == "file" && meta.is_file())
            || (target.kind == "directory" && meta.is_dir()))
        && meta.dev() == target.device
        && meta.ino() == target.inode
        && meta.uid() == target.owner
        && meta.uid() == unsafe { libc::geteuid() }
        && meta.len() == target.size
        && meta.mtime() == target.modified_seconds
        && meta.mtime_nsec() == target.modified_nanoseconds
        && (!meta.is_file() || meta.nlink() == 1)
}
#[allow(clippy::too_many_arguments)]
fn descendant_refusal(
    owner: u32,
    device: u64,
    expected_device: u64,
    regular: bool,
    links: u64,
    modified: i64,
    nanos: i64,
    created: u64,
) -> Option<&'static str> {
    if owner != unsafe { libc::geteuid() } {
        Some("descendant owned by another user")
    } else if device != expected_device {
        Some("descendant crosses a mount or device boundary")
    } else if regular && links != 1 {
        Some("hard-linked descendant")
    } else if modified < 0 || modified as u64 > created || (modified as u64 == created && nanos > 0)
    {
        Some("descendant newer than plan creation time")
    } else {
        None
    }
}

/// No-follow metadata traversal through the existing scanner. Internal links are
/// inspected as links. This path-based snapshot is not an atomic rename guard.
pub fn revalidate_unit(
    home: &Path,
    provider: &Provider,
    target: &Target,
    created: u64,
    cancel: &AtomicBool,
    budget: Budget,
) -> io::Result<()> {
    provider_config(home, provider)?;
    if target.path.parent() != Some(provider.root.as_path())
        || target.path == provider.root
        || super::providers::protected_path(&target.path)
    {
        return Err(io::Error::other(
            "Target is not an allowed top-level content unit",
        ));
    }
    super::scan::validate(&target.path)?;
    let meta = fs::symlink_metadata(&target.path)?;
    if !identity_matches(target, &meta) {
        return Err(io::Error::other("Unit identity changed or unit is unsafe"));
    }
    if meta.dev() != fs::symlink_metadata(&provider.root)?.dev() {
        return Err(io::Error::other("Unit crosses a device boundary"));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(io::Error::other("Revalidation cancelled"));
    }
    if meta.is_file() {
        return Ok(());
    }
    let row = ProviderReport {
        provider: provider.provider.clone(),
        root: target.path.clone(),
        access: "unknown".into(),
        status: Status::Complete,
        logical_bytes: None,
        allocated_bytes: None,
        files: 0,
        skipped: 0,
        skip_reasons: Default::default(),
        errors: 0,
        note: String::new(),
    };
    let mut refusal = None;
    let mut remaining = budget.global_entries;
    let report = super::scan::measure_observed(
        row,
        cancel,
        budget,
        Instant::now() + budget.total,
        &mut HashSet::new(),
        &mut remaining,
        &super::providers::protected_path,
        &mut |_| {},
        &mut |_, path, meta, reason| {
            if refusal.is_some() {
                return;
            }
            if let Some(reason) = reason
                && reason != "symlink not followed"
            {
                refusal = Some(format!("{}: {reason}", path.display()));
                return;
            }
            if let Some(m) = meta {
                if !m.is_file() && !m.is_dir() && !m.is_symlink() {
                    refusal = Some(format!("{}: special-file descendant", path.display()));
                    return;
                }
                let reason = descendant_refusal(
                    m.uid(),
                    m.dev(),
                    target.device,
                    m.is_file(),
                    m.nlink(),
                    m.mtime(),
                    m.mtime_nsec(),
                    created,
                );
                if let Some(reason) = reason {
                    refusal = Some(format!("{}: {reason}", path.display()));
                }
            }
        },
    );
    if let Some(reason) = refusal {
        return Err(io::Error::other(reason));
    }
    if report.status != Status::Complete {
        return Err(io::Error::other(format!(
            "Unit traversal incomplete: {:?}; {}",
            report.status, report.note
        )));
    }
    super::scan::validate(&target.path)?;
    if !identity_matches(target, &fs::symlink_metadata(&target.path)?) {
        return Err(io::Error::other("Unit changed during revalidation"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::{PermissionsExt, symlink},
        time::{Duration, SystemTime},
    };
    fn old(path: &Path) {
        fs::File::open(path)
            .unwrap()
            .set_times(
                fs::FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(10)),
            )
            .unwrap();
    }
    fn fixture() -> (PathBuf, crate::fixture_paths::FixtureRoot, StoredPlan) {
        let (home, cap) = crate::fixture_paths::FixtureRoot::create_test_fixture().unwrap();
        let unit = home.join("Library/Caches/ms-playwright/chromium-fixture");
        fs::create_dir_all(unit.join("Versions/A")).unwrap();
        fs::write(unit.join("Versions/A/binary"), b"fixture").unwrap();
        for p in [
            unit.join("Versions/A/binary"),
            unit.join("Versions/A"),
            unit.join("Versions"),
            unit.clone(),
        ] {
            old(&p);
        }
        let plan = super::super::plan_builder::build(
            &home,
            None,
            &["playwright-browsers".into()],
            &AtomicBool::new(false),
            Budget::quick(),
            1_000_000,
        )
        .unwrap()
        .plan;
        assert_eq!(plan.content.providers[0].targets.len(), 1);
        (home, cap, plan)
    }
    #[test]
    fn apply_revalidation_checks_age_hash_schema_and_scope_without_mutation() {
        let (home, _cap, plan) = fixture();
        fresh_plan(&plan, 1_000_600).unwrap();
        assert!(fresh_plan(&plan, 1_000_601).is_err());
        assert!(fresh_plan(&plan, 999_999).is_err());
        let mut bad = plan.clone();
        bad.schema_version = 99;
        assert!(fresh_plan(&bad, 1_000_001).is_err());
        let mut bad = plan.clone();
        bad.content.created_at_unix_seconds += 1;
        assert!(fresh_plan(&bad, 1_000_001).is_err());
        let checks = preflight(
            &home,
            &plan,
            1_000_001,
            &AtomicBool::new(false),
            Budget::quick(),
        )
        .unwrap();
        assert_eq!(checks.len(), 1);
        assert!(checks[0].refusal.is_none());
        let provider = &plan.content.providers[0];
        let target = &provider.targets[0];
        let mut bad = target.clone();
        bad.path = provider.root.clone();
        assert!(
            revalidate_unit(
                &home,
                provider,
                &bad,
                1_000_000,
                &AtomicBool::new(false),
                Budget::quick()
            )
            .is_err()
        );
        bad.path = home.join("Documents");
        assert!(
            revalidate_unit(
                &home,
                provider,
                &bad,
                1_000_000,
                &AtomicBool::new(false),
                Budget::quick()
            )
            .is_err()
        );
        let mut fake_provider = provider.clone();
        fake_provider.root = home.clone();
        assert!(provider_config(&home, &fake_provider).is_err());
        let mut changed = target.clone();
        changed.inode += 1;
        assert!(
            revalidate_unit(
                &home,
                provider,
                &changed,
                1_000_000,
                &AtomicBool::new(false),
                Budget::quick()
            )
            .is_err()
        );
        assert_eq!(
            fs::read(target.path.join("Versions/A/binary")).unwrap(),
            b"fixture"
        );
        assert!(!home.join(".optimac-rust-recovery").exists());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn apply_revalidation_refuses_new_descendants_hardlinks_and_partial_provider() {
        let (home, _cap, plan) = fixture();
        let p = &plan.content.providers[0];
        let t = &p.targets[0];
        let file = t.path.join("Versions/A/binary");
        fs::File::open(&file)
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_001)),
            )
            .unwrap();
        let check = preflight(
            &home,
            &plan,
            1_000_002,
            &AtomicBool::new(false),
            Budget::quick(),
        )
        .unwrap();
        assert!(check[0].refusal.as_ref().unwrap().contains("newer"));
        old(&file);
        fs::hard_link(&file, home.join("hard-link")).unwrap();
        assert!(
            revalidate_unit(
                &home,
                p,
                t,
                1_000_000,
                &AtomicBool::new(false),
                Budget::quick()
            )
            .unwrap_err()
            .to_string()
            .contains("hard-linked")
        );
        fs::remove_file(home.join("hard-link")).unwrap();
        fs::set_permissions(t.path.join("Versions"), fs::Permissions::from_mode(0o000)).unwrap();
        assert!(
            preflight(
                &home,
                &plan,
                1_000_002,
                &AtomicBool::new(false),
                Budget::quick()
            )
            .is_err()
        );
        fs::set_permissions(t.path.join("Versions"), fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            preflight(
                &home,
                &plan,
                1_000_002,
                &AtomicBool::new(false),
                Budget {
                    entries: 1,
                    ..Budget::quick()
                }
            )
            .is_err()
        );
        let uid = unsafe { libc::geteuid() };
        assert!(
            descendant_refusal(uid.wrapping_add(1), 1, 1, true, 1, 10, 0, 100)
                .unwrap()
                .contains("another user")
        );
        assert!(
            descendant_refusal(uid, 2, 1, false, 1, 10, 0, 100)
                .unwrap()
                .contains("boundary")
        );
        assert!(
            descendant_refusal(uid, 1, 1, false, 1, 100, 1, 100)
                .unwrap()
                .contains("newer")
        );
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn apply_revalidation_allows_internal_links_but_refuses_unit_and_ancestor_links() {
        let (home, _cap, plan) = fixture();
        let unit = plan.content.providers[0].targets[0].path.clone();
        let outside = home.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("secret"), b"untouched").unwrap();
        symlink(&outside, unit.join("external")).unwrap();
        symlink("A", unit.join("Versions/Current")).unwrap();
        old(&unit);
        old(&unit.join("Versions"));
        // Link mtime is current; capture a fresh plan time after fixture setup.
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 1;
        let plan = super::super::plan_builder::build(
            &home,
            None,
            &["playwright-browsers".into()],
            &AtomicBool::new(false),
            Budget::quick(),
            now,
        )
        .unwrap()
        .plan;
        assert_eq!(plan.content.providers[0].targets.len(), 1);
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o000)).unwrap();
        assert!(
            preflight(&home, &plan, now, &AtomicBool::new(false), Budget::quick()).unwrap()[0]
                .refusal
                .is_none()
        );
        let p = &plan.content.providers[0];
        let mut t = p.targets[0].clone();
        symlink(&unit, p.root.join("top-link")).unwrap();
        t.path = p.root.join("top-link");
        assert!(
            revalidate_unit(&home, p, &t, now, &AtomicBool::new(false), Budget::quick()).is_err()
        );
        let mut ancestor = p.clone();
        ancestor.root = p.root.join("top-link/Versions");
        t.path = ancestor.root.join("A");
        assert!(
            revalidate_unit(
                &home,
                &ancestor,
                &t,
                now,
                &AtomicBool::new(false),
                Budget::quick()
            )
            .is_err()
        );
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(fs::read(outside.join("secret")).unwrap(), b"untouched");
        fs::remove_dir_all(home).unwrap();
    }
}
