//! One metadata scan through the inventory engine. This module never writes.
use super::{Budget, ProviderReport, Status, config::Config, plan_store::*};
use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

pub const ALLOWED: &[&str] = &[
    "playwright-browsers",
    "puppeteer-browsers",
    "cypress-browsers",
    "pnpm-store",
    "npm",
    "gradle-cache",
    "cargo-registry",
    "app-user-caches",
];

#[derive(Default)]
struct Unit {
    target: Option<Target>,
    logical: u64,
    allocated: u64,
    excluded: Vec<Exclusion>,
}
#[derive(Default)]
struct Collector {
    units: BTreeMap<(String, PathBuf), BTreeMap<PathBuf, Unit>>,
}

fn identity(path: &Path, m: &fs::Metadata) -> Target {
    Target {
        path: path.into(),
        kind: if m.is_dir() { "directory" } else { "file" }.into(),
        device: m.dev(),
        inode: m.ino(),
        owner: m.uid(),
        size: m.len(),
        modified_seconds: m.mtime(),
        modified_nanoseconds: m.mtime_nsec(),
        logical_bytes: 0,
        allocated_bytes: 0,
    }
}
/// Kept pure so owner/device/time refusal can be exercised without chown or mounts.
fn refuse(
    kind: &str,
    links: u64,
    owner: u32,
    device: u64,
    expected_device: u64,
    modified: i64,
    now: u64,
) -> Option<&'static str> {
    if !matches!(kind, "file" | "directory") {
        Some("symlink or special file")
    } else if kind == "file" && links != 1 {
        Some("hard-linked file")
    } else if owner != unsafe { libc::geteuid() } {
        Some("not owned by the current user")
    } else if device != expected_device {
        Some("mount or device boundary")
    } else if modified < 0 || now.saturating_sub(modified as u64) < 86400 {
        Some("modified within 24 hours (likely in use)")
    } else {
        None
    }
}
impl Collector {
    fn observe(
        &mut self,
        row: &ProviderReport,
        path: &Path,
        meta: Option<&fs::Metadata>,
        reason: Option<&str>,
        now: u64,
    ) {
        let Ok(relative) = path.strip_prefix(&row.root) else {
            return;
        };
        let Some(first) = relative.components().next() else {
            return;
        };
        let top = row.root.join(first);
        let unit = self
            .units
            .entry((row.provider.clone(), row.root.clone()))
            .or_default()
            .entry(top.clone())
            .or_default();
        if let Some(reason) = reason {
            unit.excluded.push(Exclusion {
                path: path.into(),
                reason: reason.into(),
            });
            return;
        }
        let Some(m) = meta else { return };
        let t = identity(path, m);
        if path == top {
            unit.target = Some(t.clone());
        }
        if let Some(reason) = refuse(
            &t.kind,
            m.nlink(),
            m.uid(),
            m.dev(),
            m.dev(),
            m.mtime(),
            now,
        ) {
            unit.excluded.push(Exclusion {
                path: path.into(),
                reason: reason.into(),
            });
        }
        if m.is_file() {
            unit.logical = unit.logical.saturating_add(m.len());
            unit.allocated = unit
                .allocated
                .saturating_add(m.blocks().saturating_mul(512));
        }
    }
}

fn selection(config: Option<&Config>, selected: &[String]) -> io::Result<Config> {
    for id in selected {
        if !ALLOWED.contains(&id.as_str()) {
            return Err(io::Error::other(format!(
                "Provider {id} is outside the v1 plan allow-list"
            )));
        }
    }
    let mut value = config
        .map(|c| c.value.clone())
        .unwrap_or_else(|| toml::from_str("schema_version=2").unwrap());
    let previous = value
        .get("rule")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let default_enabled = value
        .get("defaults")
        .and_then(|d| d.get("enabled"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    let mut rules = Vec::new();
    for id in ALLOWED {
        if !selected.is_empty() && !selected.iter().any(|s| s == id) {
            continue;
        }
        let existing = previous
            .iter()
            .find(|r| r.get("id").and_then(toml::Value::as_str) == Some(id));
        let enabled = if config.is_none() {
            true
        } else {
            existing
                .and_then(|r| r.get("enabled"))
                .and_then(toml::Value::as_bool)
                .unwrap_or(default_enabled)
        };
        // Provider selection narrows configuration; it never bypasses a disabled rule.
        if !enabled {
            continue;
        }
        let mut rule = existing
            .cloned()
            .unwrap_or_else(|| toml::Value::Table(Default::default()));
        rule.as_table_mut()
            .unwrap()
            .insert("id".into(), toml::Value::String((*id).into()));
        rule.as_table_mut()
            .unwrap()
            .insert("enabled".into(), toml::Value::Boolean(true));
        rule.as_table_mut()
            .unwrap()
            .insert("operation".into(), toml::Value::String("inventory".into()));
        rules.push(rule);
    }
    value
        .as_table_mut()
        .unwrap()
        .insert("rule".into(), toml::Value::Array(rules));
    Config::parse(&toml::to_string(&value).map_err(io::Error::other)?)
}

pub struct BuiltPlan {
    pub plan: StoredPlan,
    pub scan_roots: Vec<PathBuf>,
}
pub fn build(
    home: &Path,
    config: Option<&Config>,
    selected: &[String],
    cancel: &AtomicBool,
    budget: Budget,
    now: u64,
) -> io::Result<BuiltPlan> {
    let config = selection(config, selected)?;
    for job in config.jobs(home, None, None)? {
        for root in job.roots {
            if !root.starts_with(home) || root == home || root.starts_with(home.join("OrbStack")) {
                return Err(io::Error::other(
                    "Plan root outside approved home scope or inside a known virtual mount",
                ));
            }
        }
    }
    // Check every existing ancestor below home before the inventory walk. This
    // catches cross-device roots as well as descendants; same-device firmlinks
    // remain a limit of the existing filesystem guard.
    let home_device = fs::symlink_metadata(home)?.dev();
    for job in config.jobs(home, None, None)? {
        for root in job.roots {
            let mut current = home.to_path_buf();
            for part in root
                .strip_prefix(home)
                .map_err(io::Error::other)?
                .components()
            {
                current.push(part);
                match fs::symlink_metadata(&current) {
                    Ok(m) if m.is_symlink() => break, // inventory reports the refusal
                    Ok(m) if m.dev() != home_device => {
                        return Err(io::Error::other(
                            "Plan root crosses a mount or device boundary",
                        ));
                    }
                    Ok(_) => {}
                    Err(_) => break, // inventory retains absent/denied status
                }
            }
        }
    }
    let mut collector = Collector::default();
    let inventory = super::providers::configured_inventory_observed(
        home,
        cancel,
        budget,
        &config,
        &mut |_| {},
        &mut |row, path, meta, reason| collector.observe(row, path, meta, reason, now),
    )?;
    let mut providers = Vec::new();
    let mut claimed: Vec<PathBuf> = Vec::new();
    let scan_roots = config
        .jobs(home, None, None)?
        .into_iter()
        .flat_map(|j| j.roots)
        .collect::<Vec<_>>();
    for row in &inventory.providers {
        let id = row.provider.split('/').next().unwrap_or("");
        let cap = inventory.capabilities.iter().find(|c| c.provider == id);
        let class = cap.map(|c| c.class.as_str()).unwrap_or("unknown_content");
        let risk = cap.map(|c| c.risk.as_str()).unwrap_or("protected");
        let mut p = Provider {
            provider: row.provider.clone(),
            root: row.root.clone(),
            category: super::category::NAMES[super::category::category(row)].into(),
            class: class.into(),
            risk: risk.into(),
            completeness: format!("{:?}", row.status).to_lowercase(),
            consequence:
                "Will re-download or regenerate on next use; offline use may be unavailable.".into(),
            targets: vec![],
            excluded: vec![],
        };
        let unverified = id == "npm" || row.note.to_ascii_lowercase().contains("not verified");
        let problem =
            if !ALLOWED.contains(&id) || class != "rebuildable_cache" || risk == "protected" {
                Some("provider class/risk is not eligible for v1 planning")
            } else if row.status == Status::Partial || row.note.contains("Budget reached") {
                Some("not plannable: incomplete scan, run with --deep")
            } else if row.status != Status::Complete || row.allocated_bytes.is_none() {
                Some("not plannable: missing, denied, blocked or unsupported inventory")
            } else if unverified && !selected.iter().any(|s| s == id) {
                Some("default, not verified: name this provider with --provider")
            } else if !row.root.starts_with(home) || row.root == home {
                Some("root outside approved home scope")
            } else if claimed
                .iter()
                .any(|r| row.root.starts_with(r) || r.starts_with(&row.root))
            {
                Some("overlaps another provider root")
            } else {
                None
            };
        // Roots are never target units, even when there are no eligible contents.
        p.excluded.push(Exclusion {
            path: row.root.clone(),
            reason: "provider root itself; only top-level contents may be targets".into(),
        });
        if let Some(problem) = problem {
            p.excluded.push(Exclusion {
                path: row.root.clone(),
                reason: if row.note.is_empty() {
                    problem.into()
                } else {
                    format!("{problem}; {}", row.note)
                },
            });
        }
        let mut eligible = problem.is_none();
        if eligible && let Err(error) = super::scan::validate(&row.root) {
            eligible = false;
            p.excluded.push(Exclusion {
                path: row.root.clone(),
                reason: error.to_string(),
            });
        }
        if eligible {
            claimed.push(row.root.clone());
        }
        for (path, mut unit) in collector
            .units
            .remove(&(row.provider.clone(), row.root.clone()))
            .unwrap_or_default()
        {
            if eligible && unit.excluded.is_empty() {
                if let Some(mut target) = unit.target {
                    // Metadata-only plans remain snapshots, never future mutation authority.
                    match fs::symlink_metadata(&path) {
                        Ok(m)
                            if !m.is_symlink()
                                && (m.is_dir() || m.nlink() == 1)
                                && identity(&path, &m) == target =>
                        {
                            target.logical_bytes = unit.logical;
                            target.allocated_bytes = unit.allocated;
                            p.targets.push(target);
                        }
                        _ => unit.excluded.push(Exclusion {
                            path,
                            reason: "entry changed after scan".into(),
                        }),
                    }
                } else {
                    unit.excluded.push(Exclusion {
                        path,
                        reason: "top-level identity unavailable".into(),
                    });
                }
            }
            p.excluded.extend(unit.excluded);
        }
        providers.push(p);
    }
    let present: HashSet<_> = providers.iter().map(|p| p.provider.clone()).collect();
    for id in selected {
        if !present.contains(id) {
            providers.push(Provider {
                provider: id.clone(),
                root: home.to_path_buf(),
                category: "Other".into(),
                class: "unknown_content".into(),
                risk: "review".into(),
                completeness: "not_scanned".into(),
                consequence: "No targets; provider disabled or excluded by configuration.".into(),
                targets: vec![],
                excluded: vec![Exclusion {
                    path: home.into(),
                    reason: "provider disabled or excluded by configuration".into(),
                }],
            });
        }
    }
    Ok(BuiltPlan {
        plan: StoredPlan::new(Content {
            created_at_unix_seconds: now,
            tool_version: env!("CARGO_PKG_VERSION").into(),
            providers,
        })?,
        scan_roots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::symlink,
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
    fn snapshot(root: &Path) -> Vec<(PathBuf, u64, u64, i64, i64)> {
        let mut rows = vec![];
        for e in fs::read_dir(root).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            let m = fs::symlink_metadata(&p).unwrap();
            rows.push((p.clone(), m.ino(), m.len(), m.mtime(), m.mtime_nsec()));
            if m.is_dir() {
                rows.extend(snapshot(&p));
            }
        }
        rows.sort();
        rows
    }
    #[test]
    fn immutable_plan_builds_allowed_roots_and_preserves_fixture_tree() {
        let (home, _auth) = crate::fixture_paths::FixtureRoot::create_test_fixture().unwrap();
        let ids = [
            ("playwright-browsers", "Library/Caches/ms-playwright"),
            ("puppeteer-browsers", ".cache/puppeteer"),
            ("cypress-browsers", "Library/Caches/Cypress"),
            ("gradle-cache", ".gradle/caches"),
            ("cargo-registry", ".cargo/registry/cache"),
            ("npm", ".npm/_cacache"),
        ];
        for (_, relative) in ids {
            let root = home.join(relative);
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("old"), b"fixture").unwrap();
            old(&root.join("old"));
            fs::write(root.join("recent"), b"recent").unwrap();
        }
        let before = snapshot(&home);
        for (id, _) in ids {
            let built = build(
                &home,
                None,
                &[id.into()],
                &AtomicBool::new(false),
                Budget::quick(),
                1_000_000,
            )
            .unwrap();
            assert_eq!(built.plan.content.providers.len(), 1);
            let p = &built.plan.content.providers[0];
            assert_eq!(p.targets.len(), 1, "{id}: {p:?}");
            assert_eq!(p.targets[0].logical_bytes, 7);
            assert!(p.excluded.iter().any(|e| e.reason.contains("24 hours")));
        }
        assert_eq!(snapshot(&home), before);
        let npm = build(
            &home,
            None,
            &[],
            &AtomicBool::new(false),
            Budget::quick(),
            1_000_000,
        )
        .unwrap();
        assert!(
            npm.plan
                .content
                .providers
                .iter()
                .find(|p| p.provider == "npm")
                .unwrap()
                .targets
                .is_empty()
        );
        assert!(
            build(
                &home,
                None,
                &["local-trash".into()],
                &AtomicBool::new(false),
                Budget::quick(),
                1_000_000
            )
            .is_err()
        );
        let pnpm = build(
            &home,
            None,
            &["pnpm-store".into()],
            &AtomicBool::new(false),
            Budget::quick(),
            1_000_000,
        )
        .unwrap();
        assert!(pnpm.plan.content.providers[0].targets.is_empty());
        assert_eq!(pnpm.plan.content.providers[0].completeness, "unsupported");
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn immutable_plan_excludes_unsafe_units_and_incomplete_providers() {
        let (home, _auth) = crate::fixture_paths::FixtureRoot::create_test_fixture().unwrap();
        let root = home.join("Library/Caches/ms-playwright");
        fs::create_dir_all(root.join("version")).unwrap();
        fs::write(root.join("version/file"), b"old").unwrap();
        old(&root.join("version/file"));
        old(&root.join("version"));
        let config=Config::parse("schema_version=2\n[[rule]]\nid='playwright-browsers'\nenabled=true\nexclude_name_patterns=['file']").unwrap();
        let p = build(
            &home,
            Some(&config),
            &[],
            &AtomicBool::new(false),
            Budget::quick(),
            1_000_000,
        )
        .unwrap();
        assert!(p.plan.content.providers[0].targets.is_empty());
        assert!(
            p.plan.content.providers[0]
                .excluded
                .iter()
                .any(|e| e.reason.contains("configuration"))
        );
        fs::create_dir(root.join("version/.ssh")).unwrap();
        fs::write(root.join("version/.ssh/secret"), b"fixture protected data").unwrap();
        old(&root.join("version"));
        let protected = build(
            &home,
            None,
            &["playwright-browsers".into()],
            &AtomicBool::new(false),
            Budget::quick(),
            1_000_000,
        )
        .unwrap();
        assert!(protected.plan.content.providers[0].targets.is_empty());
        assert!(
            protected.plan.content.providers[0]
                .excluded
                .iter()
                .any(|e| e.path.ends_with(".ssh") && e.reason.contains("protected"))
        );
        symlink(home.join("outside"), root.join("link")).unwrap();
        fs::hard_link(root.join("version/file"), root.join("hard")).unwrap();
        let p = build(
            &home,
            None,
            &["playwright-browsers".into()],
            &AtomicBool::new(false),
            Budget::quick(),
            1_000_000,
        )
        .unwrap();
        let p = &p.plan.content.providers[0];
        assert!(p.targets.is_empty());
        assert!(p.excluded.iter().any(|e| e.reason.contains("Symlink")));
        assert!(p.excluded.iter().any(|e| e.reason.contains("hard-linked")));
        assert!(
            p.excluded
                .iter()
                .any(|e| e.reason.contains("incomplete scan"))
        );
        let uid = unsafe { libc::geteuid() };
        assert!(
            refuse("file", 1, uid.wrapping_add(1), 1, 1, 0, 1_000_000)
                .unwrap()
                .contains("owned")
        );
        assert!(
            refuse("file", 1, uid, 2, 1, 0, 1_000_000)
                .unwrap()
                .contains("boundary")
        );
        let p = build(
            &home,
            None,
            &["playwright-browsers".into()],
            &AtomicBool::new(false),
            Budget {
                entries: 1,
                ..Budget::quick()
            },
            1_000_000,
        )
        .unwrap();
        assert!(p.plan.content.providers[0].targets.is_empty());
        fs::remove_dir_all(home).unwrap();
    }
}
