//! `clean apply <plan-id>`: move reviewed plan units into OptiMac recovery.
//!
//! Order: plan freshness and full revalidation, activity check, review grouped
//! by provider, interactive default-No confirmation, then per unit: fresh
//! revalidation and activity check, durable `validated` state, the 2A
//! directory-unit move, and a durable terminal state. Never deletes. A plan's
//! journal is created once; an applied or interrupted plan is never applied
//! again and nothing is replayed automatically.
use super::{
    Budget,
    activity::{Activity, ActivityCheck},
    apply_validation,
    plan_store::{PlanStore, Provider, StoredPlan, Target},
};
use crate::{
    fixture_paths::FixtureRoot,
    paths::AppPaths,
    recovery::{
        RecoveryContext, RecoveryPolicy,
        unit::{Faults, UnitRecord},
    },
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub const NOTICE: &str = "Moves to the OptiMac recovery folder. This does not free disk space.";
pub const SPACE_LABEL: &str = "not exact bytes freed; expected to be about zero for recovery moves";
const JOURNAL_VERSION: u32 = 1;

pub struct Env<'a> {
    pub paths: &'a AppPaths,
    pub policy: RecoveryPolicy<'a>,
    pub authority: Option<&'a FixtureRoot>,
    pub budget: Budget,
    pub cancel: &'a AtomicBool,
    pub clock: &'a dyn Fn() -> u64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub declined: bool,
    pub moved: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// Test-only interruption points; each simulates process death.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ApplyFaults {
    pub stop_before_unit: Option<usize>,
    pub stop_after_validated: Option<usize>,
    pub move_faults: Option<(usize, Faults)>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    version: u32,
    plan_id: String,
    state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recovery_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    at: u64,
}

/// Append-only journal; every line is written whole and synced before the
/// next step proceeds.
struct Journal<'a> {
    file: File,
    plan_id: &'a str,
    clock: &'a dyn Fn() -> u64,
}
impl Journal<'_> {
    fn write(
        &mut self,
        state: &str,
        unit: Option<(usize, &Path)>,
        recovery_id: Option<&str>,
        reason: Option<&str>,
    ) -> io::Result<()> {
        let mut line = serde_json::to_vec(&Event {
            version: JOURNAL_VERSION,
            plan_id: self.plan_id.into(),
            state: state.into(),
            unit: unit.map(|u| u.0),
            path: unit.map(|u| u.1.to_path_buf()),
            recovery_id: recovery_id.map(Into::into),
            reason: reason.map(Into::into),
            at: (self.clock)(),
        })?;
        line.push(b'\n');
        self.file.write_all(&line)?;
        self.file.sync_all()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitProgress {
    pub path: PathBuf,
    pub state: String,
    pub recovery_id: Option<String>,
    pub reason: Option<String>,
}
#[derive(Debug, Default)]
pub struct JournalState {
    pub applied: bool,
    pub torn_tail: bool,
    pub units: Vec<UnitProgress>,
}
impl JournalState {
    pub fn unfinished(&self) -> impl Iterator<Item = &UnitProgress> {
        self.units
            .iter()
            .filter(|u| !matches!(u.state.as_str(), "moved" | "skipped" | "failed"))
    }
}

/// Fold journal lines; an incomplete final line (interrupted write) is
/// reported, any other malformed content fails closed.
pub fn read_journal(plan_id: &str, bytes: &[u8]) -> io::Result<JournalState> {
    let mut state = JournalState::default();
    let mut lines: Vec<&[u8]> = bytes.split(|b| *b == b'\n').collect();
    if lines.last().is_some_and(|l| !l.is_empty()) {
        state.torn_tail = true;
    }
    lines.pop();
    for line in lines {
        let event: Event = serde_json::from_slice(line)
            .map_err(|e| io::Error::other(format!("Corrupt apply journal: {e}")))?;
        if event.version != JOURNAL_VERSION || event.plan_id != plan_id {
            return Err(io::Error::other("Apply journal does not match this plan"));
        }
        match (event.state.as_str(), event.unit) {
            ("started", None) => {}
            ("applied", None) => state.applied = true,
            ("planned", Some(i)) if i == state.units.len() => state.units.push(UnitProgress {
                path: event
                    .path
                    .ok_or_else(|| io::Error::other("Journal unit without path"))?,
                state: "planned".into(),
                recovery_id: None,
                reason: None,
            }),
            ("validated" | "moved" | "skipped" | "failed", Some(i)) if i < state.units.len() => {
                let unit = &mut state.units[i];
                unit.state = event.state;
                unit.recovery_id = event.recovery_id.or(unit.recovery_id.take());
                unit.reason = event.reason;
            }
            _ => return Err(io::Error::other("Unexpected apply journal event")),
        }
    }
    Ok(state)
}

fn unfinished_report(plan_id: &str, journal: &JournalState) -> String {
    let mut text = format!(
        "Plan {plan_id} has an unfinished apply. Nothing was replayed and this plan will not be applied again.\n"
    );
    if journal.torn_tail {
        text.push_str("The journal's last entry was cut off by the interruption.\n");
    }
    for unit in journal.unfinished() {
        text.push_str(&format!("  {}: {}", unit.path.display(), unit.state));
        match (&unit.recovery_id, unit.state.as_str()) {
            (Some(id), _) => text.push_str(&format!(
                " (move may have started; check: optimac recovery inspect {id})\n"
            )),
            (None, "planned") => text.push_str(" (not touched)\n"),
            _ => text.push('\n'),
        }
    }
    text.push_str(&format!(
        "Full state: optimac recovery inspect {plan_id}. Create a new plan to clean again.\n"
    ));
    text
}

struct Review<'p> {
    provider: &'p Provider,
    target: &'p Target,
    skip: Option<String>,
}

fn render_review(plan: &StoredPlan, units: &[Review], coverage: Option<String>) -> String {
    let mut out = format!(
        "Apply plan {} (created {} Unix seconds)\n{NOTICE}\n",
        plan.plan_id, plan.content.created_at_unix_seconds
    );
    let mut providers: Vec<&Provider> = plan
        .content
        .providers
        .iter()
        .filter(|p| !p.targets.is_empty())
        .collect();
    providers.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then(a.provider.cmp(&b.provider))
    });
    let mut category = "";
    for p in providers {
        if category != p.category {
            category = &p.category;
            out.push_str(&format!("\n{category}\n"));
        }
        let rows: Vec<_> = units
            .iter()
            .filter(|u| std::ptr::eq(u.provider, p))
            .collect();
        let moving: Vec<_> = rows.iter().filter(|u| u.skip.is_none()).collect();
        let allocated = moving
            .iter()
            .fold(0u64, |t, u| t.saturating_add(u.target.allocated_bytes));
        out.push_str(&format!(
            "  {}: {} unit(s) to move, {} allocated\n    Consequence: {}\n",
            p.provider,
            moving.len(),
            crate::format_size(allocated),
            p.consequence
        ));
        for u in rows {
            match &u.skip {
                None => out.push_str(&format!(
                    "    move  {} — {} allocated\n",
                    u.target.path.display(),
                    crate::format_size(u.target.allocated_bytes)
                )),
                Some(reason) => out.push_str(&format!(
                    "    skip  {} — {reason}\n",
                    u.target.path.display()
                )),
            }
        }
    }
    let moving: Vec<_> = units.iter().filter(|u| u.skip.is_none()).collect();
    if let Some(note) = coverage {
        out.push_str(&format!("\n{note}\n"));
    }
    out.push_str(&format!(
        "\nTotal: {} unit(s) to move, {} allocated; {} skipped.\n",
        moving.len(),
        crate::format_size(
            moving
                .iter()
                .fold(0u64, |t, u| t.saturating_add(u.target.allocated_bytes))
        ),
        units.len() - moving.len()
    ));
    out
}

fn activity_skip(activity: Activity) -> Option<String> {
    match activity {
        Activity::Idle => None,
        Activity::InUse(why) => Some(format!("skipped: {why}")),
        Activity::Unknown(why) => Some(format!("skipped: activity unknown ({why})")),
    }
}

fn signed_change(before: Option<u64>, after: Option<u64>) -> String {
    match (before, after) {
        (Some(b), Some(a)) if a >= b => format!("+{}", crate::format_size(a - b)),
        (Some(b), Some(a)) => format!("-{}", crate::format_size(b - a)),
        _ => "unavailable".into(),
    }
}

pub fn apply(
    env: &Env,
    plan_id: &str,
    activity: &mut dyn ActivityCheck,
    confirm: &mut dyn FnMut(&str) -> io::Result<bool>,
    out: &mut dyn Write,
) -> io::Result<Outcome> {
    apply_with(env, plan_id, activity, confirm, out, ApplyFaults::default())
}

pub(crate) fn apply_with(
    env: &Env,
    plan_id: &str,
    activity: &mut dyn ActivityCheck,
    confirm: &mut dyn FnMut(&str) -> io::Result<bool>,
    out: &mut dyn Write,
    faults: ApplyFaults,
) -> io::Result<Outcome> {
    if unsafe { libc::geteuid() } == 0 {
        return Err(io::Error::other("Do not run cleanup as root"));
    }
    let home = env.paths.home();
    let store = PlanStore::open(env.paths, &[], false)?;
    let plan = store.load(plan_id)?;
    if let Some(bytes) = store.read_apply_journal(plan_id)? {
        let journal = read_journal(plan_id, &bytes)?;
        if journal.applied {
            return Err(io::Error::other(format!(
                "Plan {plan_id} was already applied; nothing changed. Create a new plan to clean again."
            )));
        }
        return Err(io::Error::other(unfinished_report(plan_id, &journal)));
    }
    let checks = apply_validation::preflight(home, &plan, (env.clock)(), env.cancel, env.budget)?;
    let mut units = Vec::new();
    for (check, (provider, target)) in checks.into_iter().zip(
        plan.content
            .providers
            .iter()
            .flat_map(|p| p.targets.iter().map(move |t| (p, t))),
    ) {
        if check.path != target.path {
            return Err(io::Error::other("Validation order mismatch"));
        }
        let skip = if let Some(refusal) = check.refusal {
            Some(format!("skipped: {refusal}"))
        } else if target.kind != "directory" {
            Some("skipped: file units are not supported by recovery apply yet".into())
        } else {
            activity_skip(activity.check(&target.path))
        };
        units.push(Review {
            provider,
            target,
            skip,
        });
    }
    out.write_all(render_review(&plan, &units, activity.coverage()).as_bytes())?;
    let eligible = units.iter().filter(|u| u.skip.is_none()).count();
    if eligible == 0 {
        out.write_all(b"Nothing to move; nothing changed.\n")?;
        return Ok(Outcome {
            skipped: units.len(),
            ..Outcome::default()
        });
    }
    if !confirm(&format!(
        "Move {eligible} unit(s) to the OptiMac recovery folder? [y/N] "
    ))? {
        out.write_all(b"Cancelled; nothing changed.\n")?;
        return Ok(Outcome {
            declined: true,
            ..Outcome::default()
        });
    }
    // The user may have waited at the prompt; never act on an expired plan.
    apply_validation::fresh_plan(&plan, (env.clock)())
        .map_err(|e| io::Error::other(format!("{e}; nothing changed")))?;
    let context = RecoveryContext::initialize(home, env.policy)?;
    let lock = context.lock()?;
    let mut journal = Journal {
        file: store.create_apply_journal(plan_id)?,
        plan_id,
        clock: env.clock,
    };
    journal.write("started", None, None, None)?;
    for (i, u) in units.iter().enumerate() {
        journal.write("planned", Some((i, &u.target.path)), None, None)?;
    }
    let before = super::accounting::available(home);
    let mut outcome = Outcome::default();
    let mut moved_bytes = 0u64;
    let mut results = Vec::new();
    for (i, u) in units.iter().enumerate() {
        let path = u.target.path.as_path();
        if faults.stop_before_unit == Some(i) {
            return Err(io::Error::other("Injected interruption before unit"));
        }
        let mut skip = u.skip.clone();
        if skip.is_none() && env.cancel.load(Ordering::Relaxed) {
            skip = Some("skipped: cancelled before move".into());
        }
        if skip.is_none()
            && let Err(e) = apply_validation::revalidate_unit(
                home,
                u.provider,
                u.target,
                plan.content.created_at_unix_seconds,
                env.cancel,
                env.budget,
            )
        {
            skip = Some(format!("skipped: changed since review ({e})"));
        }
        if skip.is_none() {
            skip = activity_skip(activity.check(path));
        }
        if let Some(reason) = skip {
            journal.write("skipped", Some((i, path)), None, Some(&reason))?;
            outcome.skipped += 1;
            results.push(format!("  skipped  {} — {reason}", path.display()));
            continue;
        }
        let id = format!(
            "{}-{}-{i}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos(),
            std::process::id()
        );
        journal.write("validated", Some((i, path)), Some(&id), None)?;
        if faults.stop_after_validated == Some(i) {
            return Err(io::Error::other("Injected interruption after validation"));
        }
        let t = u.target;
        let record = UnitRecord::directory(
            t.path.clone(),
            &u.provider.provider,
            plan_id,
            (t.device, t.inode, t.owner),
            (t.allocated_bytes, t.logical_bytes),
            (env.clock)(),
        );
        let moved = match faults.move_faults {
            Some((at, move_faults)) if at == i => {
                context.move_directory_unit_with(
                    &lock,
                    env.authority,
                    &id,
                    &record,
                    move_faults,
                )?;
                Ok(())
            }
            _ => context.move_directory_unit(&lock, env.authority, &id, &record),
        };
        match moved {
            Ok(()) => {
                journal.write("moved", Some((i, path)), Some(&id), None)?;
                outcome.moved += 1;
                moved_bytes = moved_bytes.saturating_add(t.allocated_bytes);
                results.push(format!(
                    "  moved    {} — recovery {id}, {} allocated",
                    path.display(),
                    crate::format_size(t.allocated_bytes)
                ));
            }
            Err(e) => {
                let reason = format!("failed: {e}");
                journal.write("failed", Some((i, path)), Some(&id), Some(&reason))?;
                outcome.failed += 1;
                results.push(format!("  failed   {} — {reason}", path.display()));
            }
        }
    }
    journal.write("applied", None, None, None)?;
    drop(lock);
    let after = super::accounting::available(home);
    let mut text = format!("\nResults for plan {plan_id}\n");
    for line in results {
        text.push_str(&line);
        text.push('\n');
    }
    text.push_str(&format!(
        "Totals: {} moved ({} allocated), {} skipped, {} failed.\n\
         Observed available space change: {} ({SPACE_LABEL})\n",
        outcome.moved,
        crate::format_size(moved_bytes),
        outcome.skipped,
        outcome.failed,
        signed_change(before, after)
    ));
    if let Some(note) = activity.coverage() {
        text.push_str(&format!("{note}\n"));
    }
    text.push_str(&format!(
        "Restore from the Restore recovered items menu. Inspect: optimac recovery inspect <recovery-id> or optimac recovery inspect {plan_id}.\n"
    ));
    out.write_all(text.as_bytes())?;
    Ok(outcome)
}

/// Read-only view of a plan's apply journal and its units' recovery records.
pub fn inspect(env: &Env, plan_id: &str) -> io::Result<String> {
    let store = PlanStore::open(env.paths, &[], false)?;
    let bytes = store
        .read_apply_journal(plan_id)?
        .ok_or_else(|| io::Error::other(format!("Plan {plan_id} has no apply journal")))?;
    let journal = read_journal(plan_id, &bytes)?;
    let unfinished = journal.unfinished().count();
    let mut text = format!(
        "Plan {plan_id} apply journal: {}\n",
        if journal.applied {
            "applied (every unit reached a terminal state)".to_string()
        } else {
            format!(
                "unfinished ({unfinished} unit(s) not in a terminal state); nothing is replayed and this plan will not be applied again"
            )
        }
    );
    if journal.torn_tail {
        text.push_str("Last journal entry was cut off by an interruption.\n");
    }
    let context = if journal.units.iter().any(|u| u.recovery_id.is_some()) {
        Some(RecoveryContext::initialize(env.paths.home(), env.policy)?)
    } else {
        None
    };
    for unit in &journal.units {
        text.push_str(&format!("  {}: {}", unit.path.display(), unit.state));
        if let Some(reason) = &unit.reason {
            text.push_str(&format!(" — {reason}"));
        }
        if let (Some(id), Some(context)) = (&unit.recovery_id, &context) {
            let record = match context.inspect_directory_unit(id) {
                Ok(description) => description
                    .lines()
                    .find_map(|l| l.strip_prefix("State: "))
                    .unwrap_or("state unavailable")
                    .to_string(),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    "no recovery record (move never started)".into()
                }
                Err(e) => format!("record unreadable: {e}"),
            };
            text.push_str(&format!(" — recovery {id}: {record}"));
        }
        text.push('\n');
    }
    text.push_str("Inspection only; nothing was moved, restored or replayed.");
    Ok(text)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::cleanup::plan_builder;
    use std::{
        collections::HashMap,
        fs,
        os::unix::fs::{MetadataExt, PermissionsExt, symlink},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    /// Injected activity: a queue of answers per unit, then the last answer.
    #[derive(Default)]
    struct Scripted {
        answers: HashMap<PathBuf, Vec<Activity>>,
        calls: Vec<PathBuf>,
    }
    impl Scripted {
        fn set(&mut self, unit: &Path, answers: &[Activity]) {
            self.answers.insert(unit.to_path_buf(), answers.to_vec());
        }
    }
    impl ActivityCheck for Scripted {
        fn check(&mut self, unit: &Path) -> Activity {
            self.calls.push(unit.to_path_buf());
            match self.answers.get_mut(unit) {
                Some(queue) if queue.len() > 1 => queue.remove(0),
                Some(queue) => queue[0].clone(),
                None => Activity::Idle,
            }
        }
        fn coverage(&self) -> Option<String> {
            Some("Activity check: injected test double.".into())
        }
    }

    struct Fixture {
        home: PathBuf,
        outside: PathBuf,
        cap: FixtureRoot,
        paths: AppPaths,
        plan: StoredPlan,
        units: Vec<PathBuf>,
        now: u64,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.outside, fs::Permissions::from_mode(0o700));
            let _ = fs::remove_dir_all(&self.home);
            let _ = fs::remove_dir_all(&self.outside);
        }
    }

    fn age(path: &Path) {
        let m = fs::symlink_metadata(path).unwrap();
        if m.file_type().is_symlink() {
            return;
        }
        if m.is_dir() {
            for e in fs::read_dir(path).unwrap() {
                age(&e.unwrap().path());
            }
        }
        fs::File::open(path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(10)))
            .unwrap();
    }

    /// Bundle-style Playwright units; the first links outside the fixture.
    fn fixture(names: &[&str]) -> Fixture {
        let (home, cap) = FixtureRoot::create_test_fixture().unwrap();
        let (outside, _) = FixtureRoot::create_test_fixture().unwrap();
        fs::create_dir(outside.join("profile")).unwrap();
        fs::write(outside.join("profile/secret"), b"outside").unwrap();
        let root = home.join("Library/Caches/ms-playwright");
        let mut units = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let unit = root.join(name);
            fs::create_dir_all(unit.join("Versions/A/Resources")).unwrap();
            fs::write(unit.join("Versions/A/binary"), name.as_bytes()).unwrap();
            fs::write(unit.join("Versions/A/Resources/Info.plist"), b"plist").unwrap();
            symlink("A", unit.join("Versions/Current")).unwrap();
            symlink("Versions/Current/Resources", unit.join("Resources")).unwrap();
            if i == 0 {
                symlink(outside.join("profile"), unit.join("external-profile")).unwrap();
            }
            units.push(unit);
        }
        age(&root);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 1;
        let built = plan_builder::build(
            &home,
            None,
            &["playwright-browsers".into()],
            &AtomicBool::new(false),
            Budget::quick(),
            now,
        )
        .unwrap();
        let plan = built.plan;
        assert_eq!(plan.content.providers[0].targets.len(), names.len());
        let paths = AppPaths::from_home(Some(home.clone().into_os_string())).unwrap();
        PlanStore::open(&paths, &built.scan_roots, true)
            .unwrap()
            .save(&plan)
            .unwrap();
        units.sort();
        Fixture {
            home,
            outside,
            cap,
            paths,
            plan,
            units,
            now,
        }
    }

    /// No-follow tree snapshot (identity, link target or bytes).
    fn tree(root: &Path) -> Vec<(PathBuf, u64, String)> {
        let mut out = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(path) = pending.pop() {
            let m = fs::symlink_metadata(&path).unwrap();
            let detail = if m.file_type().is_symlink() {
                format!("link:{}", fs::read_link(&path).unwrap().display())
            } else if m.is_dir() {
                pending.extend(fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
                "dir".into()
            } else {
                format!(
                    "file:{}",
                    String::from_utf8_lossy(&fs::read(&path).unwrap())
                )
            };
            out.push((path.strip_prefix(root).unwrap().into(), m.ino(), detail));
        }
        out.sort();
        out
    }

    struct Run {
        result: io::Result<Outcome>,
        output: String,
        prompts: Vec<String>,
    }
    fn run(
        f: &Fixture,
        activity: &mut dyn ActivityCheck,
        answer: bool,
        faults: ApplyFaults,
    ) -> Run {
        let clock = move || f.now + 1;
        let cancel = AtomicBool::new(false);
        let env = Env {
            paths: &f.paths,
            policy: RecoveryPolicy::Fixture(Some(&f.cap)),
            authority: Some(&f.cap),
            budget: Budget::quick(),
            cancel: &cancel,
            clock: &clock,
        };
        let mut prompts = Vec::new();
        let mut confirm = |prompt: &str| {
            prompts.push(prompt.to_string());
            Ok(answer)
        };
        let mut output = Vec::new();
        let result = apply_with(
            &env,
            &f.plan.plan_id,
            activity,
            &mut confirm,
            &mut output,
            faults,
        );
        Run {
            result,
            output: String::from_utf8(output).unwrap(),
            prompts,
        }
    }
    fn inspect_plan(f: &Fixture) -> String {
        let clock = move || f.now + 1;
        let cancel = AtomicBool::new(false);
        inspect(
            &Env {
                paths: &f.paths,
                policy: RecoveryPolicy::Fixture(Some(&f.cap)),
                authority: Some(&f.cap),
                budget: Budget::quick(),
                cancel: &cancel,
                clock: &clock,
            },
            &f.plan.plan_id,
        )
        .unwrap()
    }
    fn journal(f: &Fixture) -> Option<JournalState> {
        PlanStore::open(&f.paths, &[], false)
            .unwrap()
            .read_apply_journal(&f.plan.plan_id)
            .unwrap()
            .map(|b| read_journal(&f.plan.plan_id, &b).unwrap())
    }
    fn context(f: &Fixture) -> RecoveryContext {
        RecoveryContext::initialize(&f.home, RecoveryPolicy::Fixture(Some(&f.cap))).unwrap()
    }
    fn recovery(f: &Fixture) -> PathBuf {
        f.home.join(".optimac-rust-recovery")
    }

    #[test]
    fn apply_declined_confirmation_changes_nothing() {
        let f = fixture(&["chromium-1248", "firefox-1509"]);
        let before: Vec<_> = f.units.iter().map(|u| tree(u)).collect();
        let r = run(&f, &mut Scripted::default(), false, ApplyFaults::default());
        println!("DECLINED_REVIEW\n{}", r.output);
        assert_eq!(
            r.result.unwrap(),
            Outcome {
                declined: true,
                ..Outcome::default()
            }
        );
        assert_eq!(
            r.prompts,
            ["Move 2 unit(s) to the OptiMac recovery folder? [y/N] "]
        );
        for needle in [
            NOTICE,
            "playwright-browsers: 2 unit(s) to move, ",
            "Consequence: ",
            &format!("    move  {} — ", f.units[0].display()),
            "Activity check: injected test double.",
            "Total: 2 unit(s) to move, ",
            "Cancelled; nothing changed.",
        ] {
            assert!(
                r.output.contains(needle),
                "missing {needle:?}\n{}",
                r.output
            );
        }
        assert!(!recovery(&f).exists());
        assert!(journal(&f).is_none());
        let after: Vec<_> = f.units.iter().map(|u| tree(u)).collect();
        assert_eq!(after, before);
    }

    #[test]
    fn apply_moves_reviewed_units_reports_results_then_restores() {
        let f = fixture(&["chromium-1248", "firefox-1509"]);
        let before: Vec<_> = f.units.iter().map(|u| tree(u)).collect();
        fs::set_permissions(&f.outside, fs::Permissions::from_mode(0o000)).unwrap();
        let r = run(&f, &mut Scripted::default(), true, ApplyFaults::default());
        println!("COMPLETED_APPLY\n{}", r.output);
        assert_eq!(
            r.result.unwrap(),
            Outcome {
                moved: 2,
                ..Outcome::default()
            }
        );
        let state = journal(&f).unwrap();
        assert!(state.applied && !state.torn_tail);
        assert_eq!(state.unfinished().count(), 0);
        for (unit, progress) in f.units.iter().zip(&state.units) {
            assert_eq!(&progress.path, unit);
            assert_eq!(progress.state, "moved");
            assert!(!unit.exists());
            let id = progress.recovery_id.as_ref().unwrap();
            assert!(
                r.output
                    .contains(&format!("  moved    {} — recovery {id}, ", unit.display()))
            );
        }
        assert!(r.output.contains("Totals: 2 moved ("));
        assert!(r.output.contains(", 0 skipped, 0 failed."));
        assert!(r.output.contains("Observed available space change: "));
        assert!(r.output.contains(&format!("({SPACE_LABEL})")));
        let inspected = inspect_plan(&f);
        println!("PLAN_INSPECT_AFTER_APPLY\n{inspected}");
        assert!(inspected.contains("applied (every unit reached a terminal state)"));
        assert_eq!(inspected.matches("— recovery ").count(), 2);
        assert_eq!(inspected.matches(": moved: payload in recovery").count(), 2);
        let ctx = context(&f);
        for (unit, progress) in f.units.iter().zip(&state.units) {
            let id = progress.recovery_id.as_ref().unwrap();
            let restored = ctx.restore_directory_unit(Some(&f.cap), id).unwrap();
            println!("RESTORE {restored}");
            assert!(unit.exists());
        }
        fs::set_permissions(&f.outside, fs::Permissions::from_mode(0o700)).unwrap();
        let after: Vec<_> = f.units.iter().map(|u| tree(u)).collect();
        assert_eq!(after, before);
        assert_eq!(
            fs::read(f.outside.join("profile/secret")).unwrap(),
            b"outside"
        );
        let inspected = inspect_plan(&f);
        println!("PLAN_INSPECT_AFTER_RESTORE\n{inspected}");
        assert_eq!(inspected.matches(": restored: payload returned").count(), 2);
        // An applied plan is never applied again, even after restore.
        let again = run(&f, &mut Scripted::default(), true, ApplyFaults::default());
        assert!(
            again
                .result
                .unwrap_err()
                .to_string()
                .contains("already applied"),
        );
        assert!(again.prompts.is_empty() && again.output.is_empty());
        assert_eq!(f.units.iter().map(|u| tree(u)).collect::<Vec<_>>(), before);
    }

    #[test]
    fn apply_skips_in_use_unknown_and_newly_active_units() {
        let f = fixture(&[
            "chromium-1248",
            "ffmpeg-1011",
            "firefox-1509",
            "webkit-2140",
        ]);
        let [chromium, ffmpeg, firefox, webkit] = [0, 1, 2, 3].map(|i| f.units[i].clone());
        let before: Vec<_> = f.units.iter().map(|u| tree(u)).collect();
        let mut activity = Scripted::default();
        activity.set(
            &chromium,
            &[Activity::InUse("in use by process 4242 (Chromium)".into())],
        );
        activity.set(
            &firefox,
            &[Activity::Unknown(
                "your process 77 could not be inspected".into(),
            )],
        );
        activity.set(
            &ffmpeg,
            &[
                Activity::Idle,
                Activity::InUse("in use by process 99 (ffmpeg)".into()),
            ],
        );
        let r = run(&f, &mut activity, true, ApplyFaults::default());
        println!("SKIPS_APPLY\n{}", r.output);
        assert_eq!(
            r.result.unwrap(),
            Outcome {
                moved: 1,
                skipped: 3,
                ..Outcome::default()
            }
        );
        assert!(r.output.contains(&format!(
            "    skip  {} — skipped: in use by process 4242 (Chromium)",
            chromium.display()
        )));
        assert!(r.output.contains(&format!(
            "    skip  {} — skipped: activity unknown (your process 77 could not be inspected)",
            firefox.display()
        )));
        assert!(r.output.contains(&format!(
            "  skipped  {} — skipped: in use by process 99 (ffmpeg)",
            ffmpeg.display()
        )));
        assert!(
            r.output
                .contains(&format!("  moved    {} — recovery ", webkit.display()))
        );
        assert!(r.output.contains("Totals: 1 moved ("));
        assert_eq!(
            r.prompts,
            ["Move 2 unit(s) to the OptiMac recovery folder? [y/N] "]
        );
        for (i, unit) in f.units.iter().enumerate() {
            if *unit == webkit {
                assert!(!unit.exists());
            } else {
                assert_eq!(tree(unit), before[i]);
            }
        }
        // Each eligible unit is checked again right before its move.
        assert_eq!(activity.calls.iter().filter(|c| **c == ffmpeg).count(), 2);
        assert_eq!(activity.calls.iter().filter(|c| **c == chromium).count(), 1);
        let state = journal(&f).unwrap();
        assert!(state.applied);
        assert_eq!(
            state
                .units
                .iter()
                .map(|u| u.state.as_str())
                .collect::<Vec<_>>(),
            ["skipped", "skipped", "skipped", "moved"]
        );
    }

    #[test]
    fn apply_interruption_leaves_readable_states_and_never_replays() {
        let f = fixture(&["chromium-1248", "firefox-1509", "webkit-2140"]);
        let before: Vec<_> = f.units.iter().map(|u| tree(u)).collect();
        let faults = ApplyFaults {
            move_faults: Some((
                1,
                Faults {
                    stop_after_record: true,
                    ..Faults::default()
                },
            )),
            ..ApplyFaults::default()
        };
        let r = run(&f, &mut Scripted::default(), true, faults);
        assert!(r.result.is_err());
        assert!(!f.units[0].exists());
        assert_eq!(tree(&f.units[1]), before[1]);
        assert_eq!(tree(&f.units[2]), before[2]);
        let state = journal(&f).unwrap();
        assert!(!state.applied);
        assert_eq!(
            state
                .units
                .iter()
                .map(|u| u.state.as_str())
                .collect::<Vec<_>>(),
            ["moved", "validated", "planned"]
        );
        let interrupted_id = state.units[1].recovery_id.clone().unwrap();
        let journal_bytes = || {
            PlanStore::open(&f.paths, &[], false)
                .unwrap()
                .read_apply_journal(&f.plan.plan_id)
                .unwrap()
                .unwrap()
        };
        let recorded = journal_bytes();
        let listing = || {
            let mut names: Vec<_> = fs::read_dir(recovery(&f))
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            names.sort();
            names
        };
        let records = listing();
        for _ in 0..2 {
            let inspected = inspect_plan(&f);
            println!("PLAN_INSPECT_INTERRUPTED\n{inspected}");
            assert!(inspected.contains("unfinished (2 unit(s) not in a terminal state)"));
            assert!(inspected.contains(&format!(
                "{}: validated — recovery {interrupted_id}: recorded_not_moved",
                f.units[1].display()
            )));
            assert!(inspected.contains(&format!("{}: planned\n", f.units[2].display())));
            let again = run(&f, &mut Scripted::default(), true, ApplyFaults::default());
            let report = again.result.unwrap_err().to_string();
            assert!(
                report.contains("unfinished apply. Nothing was replayed"),
                "{report}"
            );
            assert!(report.contains(&format!(
                "{}: validated (move may have started; check: optimac recovery inspect {interrupted_id})",
                f.units[1].display()
            )));
            assert!(report.contains(&format!("{}: planned (not touched)", f.units[2].display())));
            assert!(!report.contains(&format!("{}:", f.units[0].display())));
            assert!(again.prompts.is_empty());
        }
        assert_eq!(journal_bytes(), recorded);
        assert_eq!(listing(), records);
        assert_eq!(tree(&f.units[1]), before[1]);
        assert_eq!(tree(&f.units[2]), before[2]);
    }

    #[test]
    fn apply_partial_completion_reports_unfinished_units() {
        let f = fixture(&["chromium-1248", "firefox-1509", "webkit-2140"]);
        let before: Vec<_> = f.units.iter().map(|u| tree(u)).collect();
        let faults = ApplyFaults {
            stop_after_validated: Some(1),
            ..ApplyFaults::default()
        };
        assert!(
            run(&f, &mut Scripted::default(), true, faults)
                .result
                .is_err()
        );
        let state = journal(&f).unwrap();
        let validated = state.units[1].recovery_id.clone().unwrap();
        let inspected = inspect_plan(&f);
        assert!(inspected.contains(&format!(
            "validated — recovery {validated}: no recovery record (move never started)"
        )));
        // A torn final line (death mid-write) is reported, never guessed.
        let store = PlanStore::open(&f.paths, &[], false).unwrap();
        let journal_path = f
            .paths
            .config()
            .join("plans")
            .join(format!("{}.apply-journal", f.plan.plan_id));
        let mut bytes = store.read_apply_journal(&f.plan.plan_id).unwrap().unwrap();
        bytes.extend_from_slice(br#"{"version":1,"plan_id":"#);
        fs::write(&journal_path, &bytes).unwrap();
        let report = run(&f, &mut Scripted::default(), true, ApplyFaults::default())
            .result
            .unwrap_err()
            .to_string();
        println!("PARTIAL_REPORT\n{report}");
        assert!(report.contains("last entry was cut off"));
        assert!(report.contains(&format!("{}: validated", f.units[1].display())));
        assert!(report.contains(&format!("{}: planned (not touched)", f.units[2].display())));
        assert!(!f.units[0].exists());
        assert_eq!(tree(&f.units[1]), before[1]);
        assert_eq!(tree(&f.units[2]), before[2]);
        // Interruption before any unit leaves every unit planned and untouched.
        let g = fixture(&["chromium-1248"]);
        let faults = ApplyFaults {
            stop_before_unit: Some(0),
            ..ApplyFaults::default()
        };
        assert!(
            run(&g, &mut Scripted::default(), true, faults)
                .result
                .is_err()
        );
        assert!(inspect_plan(&g).contains(&format!("{}: planned\n", g.units[0].display())));
        assert!(g.units[0].exists());
        // Corrupt journal content fails closed.
        assert!(read_journal(&f.plan.plan_id, b"{\"version\":9}\n").is_err());
        assert!(read_journal(&"0".repeat(64), &bytes[..bytes.len() - 22]).is_err());
    }
}
