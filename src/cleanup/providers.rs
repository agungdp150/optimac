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
                if name.starts_with("com.apple.") || name.starts_with("group.com.apple.") {
                    continue;
                }
                for suffix in suffixes {
                    out.push(row(
                        &format!("{id}/{name}"),
                        if suffix.is_empty() {
                            e.path()
                        } else {
                            e.path().join(suffix)
                        },
                    ));
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
    validate(home)?;
    let deadline = Instant::now() + budget.total;
    let mut roots = Vec::new();
    // Metadata hints only: no tool execution, config evaluation or network access.
    for &(id, relative) in super::registry::ROOTS {
        match super::registry::resolve(home, id, relative, |key| std::env::var_os(key)) {
            Ok(root) => roots.push(row(id, root)),
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
                &|path| generic_claims.iter().any(|p| path.starts_with(p)),
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
    Ok(Inventory { schema_version:1,read_only:true,providers:reports,available_bytes:available(home),complete,
        capabilities:catalog(), notes:vec!["Inventory only. No cleanup targets selected; no files moved or deleted.".into(),
        "Environment overrides are honored for supported tools; other tool configuration and executable discovery are not yet supported.".into(),
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
    let jobs = config.jobs(home)?;
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
                _ => roots.push(row(&job.id, root.clone())),
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
    Ok(Inventory {schema_version:1,read_only:true,providers:reports,available_bytes:available(home),complete,capabilities:catalog(), notes:vec![
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
