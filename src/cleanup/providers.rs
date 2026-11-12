use super::*;

fn row(id: &str, root: PathBuf) -> ProviderReport {
    ProviderReport {
        provider: id.into(),
        access: "unknown".into(),
        root,
        status: Status::Complete,
        logical_bytes: None,
        allocated_bytes: None,
        files: 0,
        skipped: 0,
        errors: 0,
        note: String::new(),
    }
}
fn children(
    id: &str,
    root: PathBuf,
    suffixes: &[&str],
    out: &mut Vec<ProviderReport>,
    cancel: &AtomicBool,
    deadline: Instant,
) {
    if let Err(e) = validate(&root) {
        let mut r = row(id, root);
        r.status = status(&e);
        r.note = e.to_string();
        out.push(r);
        return;
    }
    let iter = match fs::read_dir(&root) {
        Ok(i) => i,
        Err(e) => {
            let mut r = row(id, root);
            r.status = status(&e);
            r.note = e.to_string();
            out.push(r);
            return;
        }
    };
    let mut count = 0;
    for entry in iter {
        if count >= 1000 || cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            let mut r = row(id, root.clone());
            r.status = Status::Partial;
            r.note = "Discovery incomplete".into();
            out.push(r);
            break;
        }
        count += 1;
        match entry {
            Ok(e) => {
                let name = e.file_name();
                let Some(name) = name.to_str() else {
                    let mut r = row(id, e.path());
                    r.status = Status::Blocked;
                    r.note = "Non-UTF8 name excluded".into();
                    out.push(r);
                    continue;
                };
                if name.starts_with("com.apple.")
                    || name.starts_with("group.com.apple.")
                    || protected_component(name)
                {
                    continue;
                }
                if suffixes == [""] && !e.file_type().is_ok_and(|t| t.is_dir()) {
                    continue;
                }
                for suffix in suffixes {
                    let mut item = row(
                        &format!("{id}/{name}"),
                        if suffix.is_empty() {
                            e.path()
                        } else {
                            e.path().join(suffix)
                        },
                    );
                    if id == "application-support" {
                        item.note = "User state / review; inventory only".into();
                    }
                    if id == "user-logs" {
                        item.note = "User logs / review; inventory only".into();
                    }
                    out.push(item);
                }
            }
            Err(e) => {
                let mut r = row(id, root.clone());
                r.status = Status::Partial;
                r.note = e.to_string();
                out.push(r);
            }
        }
    }
    if count == 0 && !cancel.load(Ordering::Relaxed) && Instant::now() < deadline {
        let mut r = row(id, root);
        r.note = "No child entries".into();
        r.logical_bytes = Some(0);
        r.allocated_bytes = Some(0);
        out.push(r);
    }
}
pub fn inventory(home: &Path, cancel: &AtomicBool, budget: Budget) -> io::Result<Inventory> {
    inventory_with_progress(home, cancel, budget, &mut |_| {})
}

pub fn inventory_with_progress(
    home: &Path,
    cancel: &AtomicBool,
    budget: Budget,
    progress: &mut dyn FnMut(&ProviderReport),
) -> io::Result<Inventory> {
    let pip = super::pip::resolve(home, cancel);
    let go = super::go::resolve(home, cancel);
    inventory_with_resolutions(home, cancel, budget, progress, pip, go)
}

pub(super) fn inventory_with_resolutions(
    home: &Path,
    cancel: &AtomicBool,
    budget: Budget,
    progress: &mut dyn FnMut(&ProviderReport),
    pip: super::pip::Resolution,
    go: super::go::GoRoots,
) -> io::Result<Inventory> {
    validate(home)?;
    let deadline = Instant::now() + budget.total;
    let mut roots = Vec::new();
    for &(id, relative) in super::registry::ROOTS {
        if id == "go-build" || id == "go-modules" {
            let g = if id == "go-build" {
                &go.build
            } else {
                &go.modules
            };
            let mut r = row(id, g.path.clone().unwrap_or_else(|| home.join(relative)));
            r.note = format!(
                "go {}: {}{}",
                g.status,
                g.detail,
                if id == "go-modules" {
                    " (review; never auto-selected)"
                } else {
                    " (rebuildable cache)"
                }
            );
            if g.path.is_none() {
                r.status = Status::Blocked;
            }
            roots.push(r);
            continue;
        }
        if id == "pip-cache" {
            let mut r = row(
                id,
                pip.path
                    .clone()
                    .unwrap_or_else(|| home.join("Library/Caches/pip")),
            );
            r.note = format!("pip {}: {}", pip.status, pip.detail);
            if pip.path.is_none() {
                r.status = Status::Blocked;
            }
            roots.push(r);
            continue;
        }
        match super::registry::resolve(home, id, relative, |key| std::env::var_os(key)) {
            Ok(root) => {
                let mut r = row(id, root);
                if matches!(id, "orbstack-storage" | "docker-desktop-storage") {
                    r.note = "Filesystem estimate; user state / review; no daemon contacted; not reclaimable bytes".into();
                }
                roots.push(r);
            }
            Err(e) => {
                let mut r = row(id, home.join(relative));
                r.status = Status::Blocked;
                r.note = e.into();
                roots.push(r);
            }
        }
    }
    children(
        "app-user-caches",
        home.join("Library/Caches"),
        &[""],
        &mut roots,
        cancel,
        deadline,
    );
    children(
        "sandbox-app-caches",
        home.join("Library/Containers"),
        &["Data/Library/Caches"],
        &mut roots,
        cancel,
        deadline,
    );
    children(
        "group-container-caches",
        home.join("Library/Group Containers"),
        &["Library/Caches", "Data/Library/Caches"],
        &mut roots,
        cancel,
        deadline,
    );
    children(
        "application-support",
        home.join("Library/Application Support"),
        &[""],
        &mut roots,
        cancel,
        deadline,
    );
    children(
        "user-logs",
        home.join("Library/Logs"),
        &[""],
        &mut roots,
        cancel,
        deadline,
    );
    let specific: Vec<_> = roots
        .iter()
        .filter(|r| !r.provider.contains('/'))
        .map(|r| r.root.clone())
        .collect();
    roots.retain(|r| !r.provider.starts_with("app-user-caches/") || !specific.contains(&r.root));
    super::access::preflight(&mut roots, cancel, deadline, progress);
    let mut seen = HashSet::new();
    let mut remaining = budget.global_entries;
    let mut reports = Vec::new();
    let count = roots.len();
    for (index, r) in roots.into_iter().enumerate() {
        let generic_claims = if is_generic(&r.provider) {
            specific.clone()
        } else {
            Vec::new()
        };
        let share = budget.fair_share(count - index, remaining, deadline);
        if r.status != Status::Complete || r.logical_bytes.is_some() {
            reports.push(r);
        } else {
            reports.push(measure(
                r,
                cancel,
                share,
                deadline,
                &mut seen,
                &mut remaining,
                &|path| generic_claims.iter().any(|p| path.starts_with(p)) || protected_path(path),
                progress,
            ));
        }
    }
    reports.sort_by(|a, b| {
        b.allocated_bytes
            .cmp(&a.allocated_bytes)
            .then(a.root.cmp(&b.root))
    });
    let complete = reports
        .iter()
        .all(|r| matches!(r.status, Status::Complete | Status::Absent));
    Ok(Inventory { schema_version:1,read_only:true,providers:reports,available_bytes:available(home),home_used_bytes:super::accounting::used(home),complete,
        capabilities:catalog(), notes:vec!["Inventory only. No cleanup targets selected; no files moved or deleted.".into(),
        "Pip discovery may run a fixed candidate; other tool configuration discovery remains limited to environment and default hints.".into(),
        "Apple-owned generic cache directories are excluded. Allocated bytes exclude directory metadata and may share APFS extents; neither size is guaranteed reclaimable.".into(),
        "Denied sizes are unknown. macOS privacy, ownership or ACL restrictions may limit coverage. Check System Settings > Privacy & Security > Full Disk Access for the launching terminal/app where appropriate, then restart and retry.".into(),
        "Docker Desktop storage is a filesystem estimate only: image, container, volume and build-cache usage are not separated. No Docker daemon is contacted.".into(),
        "Xcode archives, device support, simulator data and iOS backups are user state; inventory does not authorize deletion.".into(),
        "Purgeable storage: not independently quantified. Available bytes use statfs on the home volume; other-volume capacity is not aggregated.".into()] })
}

pub fn configured_inventory(
    home: &Path,
    cancel: &AtomicBool,
    budget: Budget,
    config: &config::Config,
) -> io::Result<Inventory> {
    configured_inventory_with_progress(home, cancel, budget, config, &mut |_| {})
}

pub fn configured_inventory_with_progress(
    home: &Path,
    cancel: &AtomicBool,
    budget: Budget,
    config: &config::Config,
    progress: &mut dyn FnMut(&ProviderReport),
) -> io::Result<Inventory> {
    let budget = config.scan_budget(budget)?;
    let pip_enabled = config
        .value
        .get("rule")
        .and_then(toml::Value::as_array)
        .is_some_and(|rules| {
            rules.iter().any(|r| {
                r.get("id").and_then(toml::Value::as_str) == Some("pip")
                    && r.get("enabled")
                        .or_else(|| config.value.get("defaults").and_then(|d| d.get("enabled")))
                        .and_then(toml::Value::as_bool)
                        == Some(true)
            })
        });
    let pip = pip_enabled.then(|| super::pip::resolve(home, cancel));
    let go_enabled = config
        .value
        .get("rule")
        .and_then(toml::Value::as_array)
        .is_some_and(|rules| {
            rules.iter().any(|r| {
                matches!(
                    r.get("id").and_then(toml::Value::as_str),
                    Some("go-build" | "go-modules")
                ) && r
                    .get("enabled")
                    .or_else(|| config.value.get("defaults").and_then(|d| d.get("enabled")))
                    .and_then(toml::Value::as_bool)
                    == Some(true)
            })
        });
    let go = go_enabled.then(|| super::go::resolve(home, cancel));
    let jobs = config.jobs(home, pip.as_ref(), go.as_ref())?;
    let claims: Vec<_> = super::registry::ROOTS
        .iter()
        .map(|(id, path)| {
            super::registry::resolve(home, id, path, |key| std::env::var_os(key))
                .unwrap_or_else(|_| home.join(path))
        })
        .collect();
    validate(home)?;
    let deadline = Instant::now() + budget.total;
    let mut reports = Vec::new();
    let mut seen = HashSet::new();
    let mut remaining = budget.global_entries;
    let job_count = jobs.len();
    for (job_index, job) in jobs.into_iter().enumerate() {
        let job_budget = budget.fair_share(job_count - job_index, remaining, deadline);
        let job_deadline = (Instant::now() + job_budget.per_root).min(deadline);
        let mut job_remaining = (remaining / (job_count - job_index)).min(budget.entries);
        let job_initial = job_remaining;
        let mut roots = Vec::new();
        let go_result = go.as_ref().and_then(|g| match job.id.as_str() {
            "go-build" => Some(&g.build),
            "go-modules" => Some(&g.modules),
            _ => None,
        });
        if let Some(g) = go_result
            && g.path.is_none()
        {
            let mut r = row(&job.id, home.to_path_buf());
            r.status = Status::Blocked;
            r.note = format!("go {}: {}", g.status, g.detail);
            reports.push(r);
            continue;
        }
        if job.id == "pip" && pip.as_ref().is_some_and(|p| p.path.is_none()) {
            let mut r = row("pip", home.join("Library/Caches/pip"));
            r.status = Status::Blocked;
            let p = pip.as_ref().unwrap();
            r.note = format!("pip {}: {}", p.status, p.detail);
            reports.push(r);
            continue;
        }
        if let Some(note) = job.unsupported {
            let mut r = row(&job.id, home.to_path_buf());
            r.status = if note.starts_with("Provider discovery") {
                if catalog()
                    .iter()
                    .any(|p| p.provider == job.id && p.certification == "guidance_only")
                {
                    Status::Guidance
                } else {
                    Status::Unsupported
                }
            } else {
                Status::Blocked
            };
            r.note = note;
            reports.push(r);
            continue;
        }
        for root in &job.roots {
            if job.excluded(root) {
                continue;
            }
            match job.id.as_str() {
                "project-artifacts" => {
                    let (outputs, complete) =
                        super::projects::discover(root, &job, job_budget, cancel, deadline);
                    for path in outputs {
                        let mut report = row(&job.id, path);
                        report.note = "Recognized project/output relationship; metadata inventory only. Tracked files, dependencies and activity are not certified for removal.".into();
                        roots.push(report);
                    }
                    if !complete {
                        let mut report = row(&job.id, root.clone());
                        report.status = if cancel.load(Ordering::Relaxed) {
                            Status::Cancelled
                        } else {
                            Status::Partial
                        };
                        report.note = "Project discovery incomplete; unknown outputs remain".into();
                        roots.push(report);
                    }
                }
                "application-support" | "user-logs" => {
                    children(&job.id, root.clone(), &[""], &mut roots, cancel, deadline)
                }
                "app-user-caches" => {
                    children(&job.id, root.clone(), &[""], &mut roots, cancel, deadline)
                }
                "sandbox-app-caches" => children(
                    &job.id,
                    root.clone(),
                    &["Data/Library/Caches"],
                    &mut roots,
                    cancel,
                    deadline,
                ),
                "group-container-caches" => children(
                    &job.id,
                    root.clone(),
                    &["Library/Caches", "Data/Library/Caches"],
                    &mut roots,
                    cancel,
                    deadline,
                ),
                _ => {
                    let mut r = row(&job.id, root.clone());
                    if let Some(g) = go_result {
                        r.note = format!(
                            "go {}: {}{}",
                            g.status,
                            g.detail,
                            if job.id == "go-modules" {
                                " (review; never auto-selected)"
                            } else {
                                " (rebuildable cache)"
                            }
                        );
                    }
                    if job.id == "orbstack-storage" {
                        r.note = "Filesystem estimate; user state / review; no daemon contacted; not reclaimable bytes".into();
                    }
                    if job.id == "pip" {
                        let p = pip.as_ref().unwrap();
                        r.note = format!("pip {}: {}", p.status, p.detail);
                    }
                    roots.push(r);
                }
            }
        }
        if roots.is_empty() {
            let mut r = row(&job.id, home.to_path_buf());
            r.note = "No effective roots (empty scope or excluded)".into();
            reports.push(r);
        }
        super::access::preflight(&mut roots, cancel, deadline, progress);
        let count = roots.len();
        for (index, r) in roots.into_iter().enumerate() {
            let share = job_budget.fair_share(count - index, job_remaining, job_deadline);
            if is_generic(&job.id) && claims.iter().any(|p| r.root.starts_with(p)) {
                let mut r = r;
                r.status = Status::Blocked;
                r.note = "Owned by a specific provider; generic inventory cannot override its scope or exclusions".into();
                reports.push(r);
                continue;
            }
            if job.excluded(&r.root) {
                continue;
            }
            if r.status != Status::Complete || r.logical_bytes.is_some() {
                reports.push(r);
            } else {
                reports.push(measure(
                    r,
                    cancel,
                    share,
                    deadline,
                    &mut seen,
                    &mut job_remaining,
                    &|p| {
                        job.excluded(p)
                            || protected_path(p)
                            || (is_generic(&job.id) && claims.iter().any(|c| p.starts_with(c)))
                    },
                    progress,
                ));
            }
        }
        remaining = remaining.saturating_sub(job_initial - job_remaining);
    }
    reports.sort_by(|a, b| {
        b.allocated_bytes
            .cmp(&a.allocated_bytes)
            .then(a.root.cmp(&b.root))
    });
    let complete = reports
        .iter()
        .all(|r| matches!(r.status, Status::Complete | Status::Absent));
    Ok(Inventory {schema_version:1,read_only:true,providers:reports,available_bytes:available(home),home_used_bytes:super::accounting::used(home),complete,capabilities:catalog(), notes:vec![
        "Configured read-only inventory. Enabled rules only; no operations execute.".into(),
        "Global and per-rule exclusions applied before traversal. Unsupported providers are reported as blocked.".into(),
        "Custom file/project scopes report metadata totals, not duplicate detection or artifact eligibility. Sizes are not reclaimable-space estimates.".into(),
        "Age/size thresholds affect future recommendations, not inventory visibility. Purgeable storage is not independently quantified.".into()]})
}

fn is_generic(id: &str) -> bool {
    matches!(
        id.split('/').next().unwrap_or(id),
        "app-user-caches" | "sandbox-app-caches" | "group-container-caches"
    )
}

fn protected_component(name: &str) -> bool {
    matches!(
        name,
        "Mail"
            | "Messages"
            | "Notes"
            | "Keychains"
            | "CloudStorage"
            | "Mobile Documents"
            | ".ssh"
            | ".aws"
            | ".gnupg"
            | ".git"
    ) || name.starts_with(".env")
        || name.ends_with(".photoslibrary")
}
fn protected_path(path: &Path) -> bool {
    path.components()
        .any(|c| c.as_os_str().to_str().is_none_or(protected_component))
}
