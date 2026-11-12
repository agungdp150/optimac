# Recommendation-based auto-selection plan

Status: proposal, not implemented. Revised 2026-11-13 after external research.

Evidence: [`reports/macOS cleanup auto selection safety.md`](../reports/macOS%20cleanup%20auto%20selection%20safety.md)
(notes in `research_notes/macOS cleanup auto selection safety/`). Section
references below such as "Report (a)" point to that report's tiers.

## Problem

In **Clean → Run cleanup**, eligible old cache files are preselected, but every
personal file (Documents, Downloads, Desktop, Pictures, Movies, Music) must be
opened group by group and toggled with `Space`. The CLI path (`clean plan` →
`clean apply PLAN-ID`) has no per-item step, so this plan covers the TUI only.

## Goal

Preselect only what OptiMac can justify with evidence, show the reason on every
row, and keep `r` review + `y` confirm as the only thing that authorizes a move.

## What the research changed

| Earlier draft | Evidence | Now |
|---|---|---|
| Any exact duplicate preselected | Gemini 2 preselects duplicates only in Downloads and Desktop, never Documents (Report (e), MacPaw) | Preselect a copy only if it sits in Downloads/Desktop; elsewhere Suggested |
| Byte-compare every same-size pair | Reading an iCloud/File Provider placeholder downloads it; `stat` can materialize folders (Report §Duplicates, TN3150, Oakley) | Disable materialization for the process; skip `SF_DATALESS` files entirely |
| Size = logical size | APFS clones share blocks; Finder "On Disk" is wrong (Oakley 2024/2026, `getattrlist(2)`) | Report `ATTR_CMNEXT_PRIVATESIZE`; full clones get no recommendation |
| Same data bytes = same file | Tags, WhereFroms, resource forks live in xattrs (Oakley 2026) | Different resource fork = not a duplicate; keeper prefers tagged copy |
| Existing cache preselection is fine | uv: "never safe to modify the cache directly"; Cargo `registry/cache` needs re-download; `com.apple.*` caches excluded by experts; owner must not be running (Report (a)/(b)) | Fix current preselection first (Phase 0) |
| Installers: Suggested | Report agrees; promotion only with "app already installed" evidence | Unchanged; promotion stays an open decision |
| (not covered) | Root buys almost nothing; in-app password fields copy macOS stealers (Report (d)) | No elevation in this plan; nothing needing admin is ever recommended |

## Phase 0 — fix what is preselected today

`recovery::cache_roots` (`src/recovery/mod.rs:73`) feeds
`Candidate::is_cache_candidate()`, and every eligible file under it is
preselected (`initialize_cache_selection`, `src/ui/mod.rs:182`). Against the
evidence:

| Root | Today | Change | Evidence |
|---|---|---|---|
| `~/.cache/uv` | Preselected, moved file by file | **Remove from `cache_roots`.** Offer `uv cache prune` as guidance text only | uv docs: never modify directly; prune takes a lock |
| `~/.cargo/registry/cache` | Preselected | Suggested (needs re-download). Note: `registry/src` and `git/checkouts` are the safe-to-stage parts; adding them is a separate change | Cargo Book |
| `~/Library/Caches/com.apple.Safari` | Preselected | Suggested; never preselect `com.apple.*` | Report (a) exclusions |
| Chrome / Firefox / Brave / Edge caches | Preselected | Preselect only if the browser has no open files under that root; otherwise Suggested with "Chrome appears to be running" | Report (a) "owner not running"; Report (c) running apps |
| `~/.cache/go-build`, `~/.cache/pip`, `~/.npm/_cacache` | Preselected | Keep, plus the same open-files gate | Go, pip, npm docs (Report (a)) |

The open-files gate reuses `cleanup::activity::ProcessActivity`
(`src/cleanup/activity.rs`), which already lists the user's processes' open
vnodes via libproc without elevated rights. Run it once per cache root in the
scan job; `InUse` or `Unknown` downgrades that root's files to Suggested.

Also: the existing **Duplicates** action (`actions::run_with_paths`,
`Action::Duplicates`) reads file contents without a dataless check, so on a Mac
with Desktop & Documents in iCloud it can trigger downloads today. Phase 1's
helper fixes that for both callers.

## Recommendation tiers

| Tier | Preselected | Rule | Report |
|---|---|---|---|
| **Recommended: old cache** | Yes | Phase 0 roots, ≥30 days, owner idle | (a) |
| **Recommended: duplicate in Downloads/Desktop** | Yes | Exact duplicate (see pipeline) located under `~/Downloads` or `~/Desktop`, keeper kept, `PRIVATESIZE > 0` | (e) |
| **Suggested: duplicate elsewhere** | No | Same evidence, but the copy is in Documents/Pictures/Movies/Music | (b) last row |
| **Suggested: cache, owner running / Apple / re-download** | No | Phase 0 downgrades | (a)/(b) |
| **Suggested: old installer** | No | `.dmg`/`.pkg` under `~/Downloads`, mtime ≥ 30 days | (b) first row |
| **Suggested: extracted archive** | No | `foo.zip`/`.tar.gz` under `~/Downloads` with sibling dir `foo/` | design inference |
| **Suggested: stale large download** | No | Under `~/Downloads`, ≥ 100 MB, mtime ≥ 180 days | design inference |
| *None* | No | Everything else, including any dataless file and any clone with no private bytes | (e) |

Age, size, type and location alone never preselect; that keeps
`docs/CLEANUP-TECH-SPEC.md`'s rule that age-based suggestions never preselect.

## Duplicate pipeline (Report (e))

New `pub(crate) fn duplicate_groups(...)` in `src/actions/mod.rs`, used by both
the Duplicates action and the recommender:

1. **Process setup:** call `setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES,
   IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF)` before scanning.
   The man page claims OFF is the default but observed behavior disagrees, so
   set it explicitly. Treat the resulting read error (reportedly `ETIMEDOUT`,
   unconfirmed) as "skipped: cloud placeholder".
2. **Skip** any file with `SF_DATALESS` (`0x40000000`) in `st_flags`. Never
   hash, count or recommend it.
3. Group by size.
4. Collapse `(dev, ino)`. Files with `nlink > 1` are already excluded by
   `validate_candidate`; keep that.
5. Read `ATTR_CMNEXT_CLONEID` and `ATTR_CMNEXT_PRIVATESIZE` via
   `getattrlist`. Members sharing a CLONEID are one storage unit: no
   recommendation (moving one frees nothing even after purge).
6. Compare a 4 KiB prefix, then full bytes (existing `equal`, budgeted).
   Also compare the `com.apple.ResourceFork` xattr; differing → not duplicates.
7. Group transitively by comparing each file with the group's first member.
8. Any limit, cancel or read error → group marked incomplete → no
   recommendation, note shown.

### Keeper rule

In order (dupeGuru/rmlint/Czkawka practice plus Oakley on xattrs):

1. Not under `~/Downloads` or `~/Desktop`.
2. Has Finder tags, a Finder comment or `kMDItemWhereFroms`.
3. Name without ` copy`, ` (1)`, ` 2` style suffixes.
4. Oldest mtime.
5. Shortest path, then lexicographic.

The keeper is never auto-selected. Selecting every copy manually shows "All
copies of this file are selected. No copy will remain in place." Do not copy
tags or quarantine between files in v1.

### Move-time re-check

`execute_with_progress` already re-validates each moved file's identity
(`validate_candidate`: dev, ino, size, mtime, single link, owner). For items
recommended as duplicates, also re-check that the keeper still exists, is not
dataless, and has the same identity it had at scan time; otherwise skip with
"duplicate keeper changed". This needs the reason passed into the reviewed
plan (see Design §6).

## Design

### 1. `duplicate_groups` (`src/actions/mod.rs`)

```rust
pub(crate) struct DuplicateGroup { pub files: Vec<PathBuf>, pub private_bytes: Vec<u64> }
pub(crate) struct DuplicateGroups { pub groups: Vec<DuplicateGroup>, pub complete: bool,
                                    pub skipped_dataless: usize, pub skipped_clones: usize }
pub(crate) fn duplicate_groups(paths: &[(PathBuf, u64)], cancel: &AtomicBool,
                               deadline: Instant) -> DuplicateGroups
```

The Duplicates action keeps its output format, adding counts of skipped cloud
placeholders and clones. Put the macOS calls (`setiopolicy_np`, `getattrlist`,
`st_flags`) in a small `src/common/apfs.rs` with fixture-friendly fallbacks for
tests.

### 2. Recommendation engine (new `src/recovery/recommend.rs`)

```rust
pub enum Tier { Recommended, Suggested }
pub enum Reason {
    OldCache,
    CacheOwnerActive { detail: String },  // Phase 0 downgrade
    AppleCache,                            // com.apple.*
    NeedsRedownload,                       // cargo registry/cache
    Duplicate { keep: PathBuf, keep_identity: (u64, u64, u64, i64) },
    OldInstaller,
    ExtractedArchive { folder: PathBuf },
    StaleLargeDownload,
}
pub struct Recommendation { pub tier: Tier, pub reason: Reason, pub frees_after_purge: u64 }
pub struct Recommendations { pub by_path: BTreeMap<PathBuf, Recommendation>, pub notes: Vec<String> }
pub fn recommend(home: &Path, plan: &Plan, now: i64, activity: &mut dyn ActivityCheck,
                 cancel: &AtomicBool) -> Recommendations
```

Pure over `Plan` plus injected activity; never adds paths, so everything still
passes `validate_candidate` and `sensitive()`.

### 3. Compute with the scan (`src/ui/mod.rs`)

Add `recommendations: Arc<Mutex<Option<Recommendations>>>` to `App`. In the
`Action::Clean` job (~line 592) call `recommend` right after `review_files`,
same thread and cancel flag; progress text "Checking caches and duplicates…".
Reset alongside `selected_files` (~line 584). Not persisted.

### 4. Preselection and keys

- `initialize_cache_selection` → `initialize_recommended_selection`: insert every
  `Tier::Recommended` path once (guard renamed `selection_initialized`).
- `a`: select all Recommended. `S`: add all Suggested. `u`: clear. `Space`:
  toggle. Manual choices are never overridden.

### 5. Rendering (`src/ui/render.rs`)

- Summary: `42 selected · frees 3.0 GB after purge (recommended 40, suggested 7)`
  and `Moving to recovery frees nothing until you purge it.` Use
  `frees_after_purge` (PRIVATESIZE), not logical size, when available.
- Group rows add `· 14 recommended`. File rows add `★ duplicate of …`,
  `◦ old installer`, `◦ Chrome is running`.
- A virtual **Recommended** group first in the list collects all
  Recommended + Suggested items.
- Notes line: skipped cloud placeholders, skipped clones, incomplete checks.
- For caches, label as tidy-up, not speed-up (Oakley: flushing frees
  "considerably less" than reported and can slow Finder/QuickLook).

### 6. Review screen and execution

`select_files` carries the recommendation per path so `describe` prints the
reason (or "manually selected") and the duplicate re-check in execution can
see the keeper identity. Execution otherwise unchanged.

## Privileges and passwords (out of scope, rule recorded)

Nothing this plan recommends needs admin rights, and nothing needing admin will
ever be Recommended (Report (d)). OptiMac will not add a password field or
pipe a password to `sudo -S`: Apple DTS rules it out, it fits CWE-256/522, and
in-app password prompts are the 2025–2026 macOS stealer pattern (ClickLock,
SHub). `defaults.allow_sudo` stays rejected (`src/cleanup/config.rs:111`). If
a privileged action is ever added (the only worthwhile one is
`tmutil thinlocalsnapshots`), the order is: print the exact command for the
user; or exec `sudo` with a fixed argv on the user's TTY so macOS prompts
(Touch ID via `sudo_local`). That would be its own proposal.

## Invariants

- Moves go to recovery and are restorable; no permanent deletion; no owner
  commands (`brew cleanup`, `uv cache prune`, …) run from this feature.
- `r` review + `y` confirm always required.
- No new paths enter the plan; `sensitive()` exclusions unchanged (already
  covers `.app`, `.photoslibrary`, `CloudStorage`, `Mobile Documents`, `.git`,
  `.env*`, Keychains).
- Dataless files are never read, hashed or recommended.
- Each duplicate group keeps one copy that is never auto-selected.
- Incomplete evidence or an active/unknown owner → never Recommended.
- Age/size/type/location alone → at most Suggested.

## Tests

`src/recovery/recommend.rs`:
- Duplicate in Downloads with keeper in Documents → copy Recommended, keeper
  untouched; both copies in Documents → Suggested.
- Keeper order: tagged beats untagged; `report.pdf` beats `report (1).pdf`.
- Same size, different bytes; differing resource fork → none.
- Clone pair (shared CLONEID / PRIVATESIZE 0) → none, counted as skipped.
- Dataless file (fixture flag via injected stat) → never read, none.
- Budget exhausted / cancel → no duplicate recommendations, note present.
- Cache root with `InUse` activity → Suggested `CacheOwnerActive`.
- `com.apple.Safari` → Suggested; `~/.cache/uv` → not in plan.
- Installer ≥ 30 days → Suggested only.

`src/actions/mod.rs`: `duplicate_groups` transitive grouping; Duplicates
output unchanged apart from the new skipped counts.

`src/ui/tests.rs`: initial selection = Recommended only; `a`/`S`/`u`; keeper
manual select → warning on review; reasons rendered; "frees nothing until
purge" line present.

`tests/cli.rs` / sandbox scripts: confirm no uv-cache paths in Clean plans.

Register new tests in `docs/TEST-CLASSIFICATION.json`; run
`scripts/check-test-classification.py` and `scripts/test-safe.sh`.

## Docs to update with the behavior change

`README.md` (How cleanup works step 2, key table, Clean row, safety list),
`docs/MENU-CONSOLIDATION-PLAN.md` step 5, `docs/CLEANUP-TECH-SPEC.md` (TUI
personal preselection limited to Downloads/Desktop exact duplicates),
`actions::description(Action::Clean)`, the hint at `src/ui/mod.rs:596`,
`CHANGELOG.md`. Consider publishing the rule table with reasons and sources,
which the report recommends as the product's trust signal.

## Delivery order

0. Phase 0 cache fixes (uv removed, Safari/cargo downgraded, open-files gate).
   Behavior change, small, ship first.
1. `common/apfs.rs` + `duplicate_groups` (dataless, clones, resource fork);
   Duplicates action switches to it.
2. `recovery::recommend` + unit tests.
3. TUI wiring and preselection; keys `a`/`S`.
4. Rendering: reasons, purge-aware sizes, Recommended group, notes.
5. Review reasons, keeper re-check, all-copies warning.
6. Docs and changelog.

## Open decisions

1. **Installers:** keep Suggested, or promote a `.dmg`/`.pkg` to Recommended
   when its app is already installed? The report names this the strongest
   promotion candidate; matching a disk image to an installed app needs a
   reliable rule (bundle ID inside the image would mean mounting it).
2. **Browser caches when the browser is closed:** keep preselecting (current
   behavior, gated), or make them Suggested since Oakley finds flushing low
   value?
3. **Turning it off:** session toggle only for v1, or a persisted setting
   (needs a config schema change)?

## Still unverified (from the report)

- Error code returned when dataless materialization is off.
- Whether `ATTR_CMNEXT_*` attributes exist on every supported macOS version
  (check against the minimum deployment target before relying on them).
- Behavior of `ProcessActivity` for browsers that keep cache files closed
  between reads (gate may report idle while the browser is open).

## Out of scope

CLI `clean plan`/`clean apply`; owner prune commands; similar/near-duplicate
media; iOS backups, Xcode Archives, Containers, Mail/Messages/Photos stores
(Report (c)); snapshots and anything needing admin; permanent purge of
recommended items.
