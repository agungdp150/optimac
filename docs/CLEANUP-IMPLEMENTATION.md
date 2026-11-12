# Cleanup implementation tracker

This is the execution order and current status. The technical spec and adapter contract define behavior; this tracker records what actually ships.

## Step 1 — organize and establish the baseline

Implemented:

- `cleanup/providers.rs`: inventory discovery and orchestration.
- `cleanup/scan.rs`: bounded metadata traversal and root validation.
- `cleanup/accounting.rs`: home-volume available-space sampling.
- `cleanup/report.rs`: public report types, budgets and text formatting.
- `cleanup/tests.rs`: disposable inventory fixtures.
- `recovery/purge.rs`: experimental individual regular-file purge with confirmation in the CLI.

The purge implementation is not a completed Phase 2 engine. It has a happy-path fixture and stale/cancelled-plan checks, but still needs descriptor-relative end-to-end access, race/fault tests and explicit interrupted-intent reconciliation. An empty lsof result is not a guarantee that a writer cannot open the file afterward. Do not extend this path to directories or bulk operations before those gates pass.

## Step 2 — finish inventory (Phase 1)

- Implemented: schema-v2 parsing and `config check` (further validation hardening remains).
- Implemented: complete catalog in JSON reports, metadata/explicit-scope/unsupported discovery capabilities, and explicit unsupported/guidance outcomes for configured providers.
- Implemented: CLI `clean scan --config`, enabled-rule filtering, explicit file/project metadata scopes and global/per-rule exclusions. TUI config loading and configured scan budgets are also implemented. Recognized Cargo, Node, SwiftPM and Gradle output inventory is implemented in explicit project scopes. Tool-specific config discovery and mutation eligibility remain pending.
- Implemented: reserved entry/time shares for remaining roots/providers, readable/absent/denied/unknown root observations, descendant error accounting, and cooperative 100 ms CLI/TUI progress callbacks. Dedicated preflight/retry UX remains pending.
- Implemented: disposable release benchmark and JSON field/status compatibility assertions. Apple Silicon measurements are recorded in CLEANUP-INVENTORY-BENCHMARK.md; Intel and input-to-frame/cancellation latency validation remain pending.

## Step 3 — complete recovery/reclamation (Phase 2)

- Descriptor-relative staging, durable versioned journals and reconciliation.
- Selected-item, plan and bulk recovery management with reviewed snapshots.
- Directory and local Trash removal only after adversarial fixture tests.
- Observed before/after volume accounting and TUI review/result integration.

## Step 4 — tool adapters (Phase 3)

Certify developer-cache and project-artifact adapters individually against their version, execution, activity and reconciliation contracts. Unsupported adapters remain inventory/guidance only.

## Step 5 — Docker and advanced Apple inventory (Phase 4)

Implement pinned local Docker Desktop context checks and reviewed object operations; retain dedicated review/guidance for archives, backups and simulator state.

## Step 6 — application coverage (Phase 5)

Add verified app/version-specific cache adapters, shared-owner checks and integration tests. Generic inventory must never bypass specific-provider restrictions.

## Phase 0 evidence

The reported 20 GB versus 2 GB comparison is unverified; see CLEANUP-BASELINE.md. Fixture measurements and the original tool's report are separate evidence. Do not call the comparison complete or claim equivalent savings without comparable measurements.

## Completion rule

Each step needs its implementation, tests, documentation and applicable release checks. A passing test suite does not imply all phases are finished. Tests must use disposable data; no user-cache deletion is part of implementation validation.

### Phase 2 progress

Regular-file purge now uses no-follow descriptor-relative opens for its payload,
intent and completion files after opening the recovery directory component by
component. `recovery inspect <id>` checks intent format and reports observed
payload/journal states without replaying deletion. Tests cover stale/cancelled
plans, completed purge, missing completion markers and malformed intents.
This is not automatic repair: staging, full race/fault testing, directory/bulk
purge, Trash management and TUI integration remain outstanding.

### Infrastructure progress

- Configured provider scans now use the same safe environment-root resolver as default scans. Invalid overrides are blocked, and explicit scan roots must narrow the resolved root.
- `clean providers [--json]` exposes every shipped catalog entry with class, risk, certification state and tested-version list. All mutation capabilities remain uncertified.
- The command runner returns separate bounded streams, exit code, duration, timeout/cancellation and truncation observations. It uses absolute executable paths, explicit environment, explicit working directory and graceful process-group termination followed by escalation after two seconds. Timeout retains captured output. This runner does not itself establish executable trust or certify adapters.
- Disposable subprocess tests cover separate output, nonzero exit, timeout output retention, output limits and refusal to search PATH.

Remaining work above is still outstanding; these infrastructure changes do not complete the five-phase specification.

### Step 2 session update

- Added bounded per-root access preflight, permission guidance and retry via `clean retry` or `r` in the inventory TUI. A denial does not establish whether Full Disk Access is missing. Descendant denials still result in partial scans.
- Generic cache scans remain read-only and now exclude or block paths claimed by specific providers, including when the specific provider is disabled in the overlay. Apple generic names remain excluded. Disposable fixtures cover this boundary and absent-root preflight.
- Project artifact defaults remain 90 days and 100,000,000 bytes in the normative spec and example; the class/risk mapping is documented in the spec's normative schema table. Inventory does not apply these thresholds.
- **Pending:** command-based pip/Go/Homebrew/npm/Yarn/Poetry/Composer/CocoaPods discovery and missing/unsupported-version reporting. Executable trust enrollment, interpreter validation and version-specific parsers required by the adapter contract have not been implemented. Existing CARGO_HOME/GRADLE_USER_HOME and other environment overrides remain metadata hints, not authoritative tool configuration discovery.
- **Pending:** real denied-access and TUI retry integration coverage, overlapping nested roots beyond the tested generic/specific boundary, Intel benchmark and UI input-to-frame/cancellation timing. This environment provides Apple Silicon only; timing and macOS privacy validation have not been measured. Step 2 remains incomplete.

### Step 2 follow-up

- Added disposable fixture coverage for a denied root becoming readable on a new scan and a root disappearing between preflight and measurement. The denial assertion is skipped when tests run as root, which can bypass mode permissions.
- Pending: trusted executable enrollment and command discovery, TUI retry integration, and scan cancellation/input-to-frame latency measurements. No executable is enrolled or run by this change. Step 2 is not complete.
- Intel benchmark remains pending: this host is Apple Silicon; run `cargo run --release --locked --example inventory_benchmark` on an Intel macOS machine, record the emitted architecture and all Quick/Deep samples in `docs/CLEANUP-INVENTORY-BENCHMARK.md`, and report OS/hardware and any incomplete scans. This disposable benchmark never scans user caches.

### Executable trust work in progress

- Added a fixed, metadata-only candidate list for the ten requested provider families in `src/cleanup/trust.rs`, with a test that candidates are absolute and unknown providers are rejected. Listing a candidate does not verify, enroll, or execute it; symlinks, scripts and untrusted binaries remain ineligible for execution until the trust contract is implemented.
- **Not implemented:** persistent trust store, identity/hash/signature verification, interactive enrollment/version probe, listing/revocation CLI, and the requested adversarial enrollment tests. Do not treat the candidate list as a trust layer. Command discovery, TUI retry integration, latency measurements and Intel benchmarks also remain pending. No tool executables are run by this work.

### Trust store schema groundwork

Added non-executing `TrustRecord`/`TrustStore` types and a bounded, strict JSON parser that rejects malformed records, duplicate/unknown providers and unsupported schema versions. This is not a usable trust store or execution authorization: secure atomic persistence, identity and SHA-256 capture, signature verification, path/permission checks, `verify`, and the requested fixture matrix remain unimplemented. CLI, adversarial tests, and command discovery also remain pending. No candidate is executed.

### Trust library follow-up

Added an in-memory schema serialization round-trip test and malformed/duplicate record cases. Atomic persistence, no-follow SHA-256 capture, verification and signature checks remain unimplemented. No trust store file is written and no executable is run. These are prerequisites before any enrollment or discovery. Existing changes span untracked cleanup files and other pre-existing modifications; no isolated implementation milestone was completed or committed in this follow-up.

### Inventory discovery policy change

Pip/Go read-only inventory will use fixed absolute candidates, reject final symlinks and files or ancestors writable by other users, prefer explicit environment settings and use bounded subprocesses with a neutral directory and explicit environment. Full executable trust enrollment is deferred until before Step 3 enables new deletion operations. The lightweight checks do not establish authenticity. Implementation and fixture tests are pending.

The pip/Go command resolver is **not yet implemented**. Existing scans still use environment/default metadata hints and do not report tool probe status. Before adding probes, isolate tests from actual installed tools, validate every returned root and provide explicit unsupported/failed outcomes. The new inventory policy changes no runtime behavior or mutation authorization.

### Pip resolver work in progress

Added a test-only candidate permission-check prototype and a disposable fixture for symlink and writable-parent refusal. It is not connected to scans and does not run pip. Pip root command resolution, per-tool statuses and the remaining requested fixture cases are still unimplemented; the previous environment/default behavior remains in use. Go is unchanged.

### Pip inventory resolver

Default and enabled configured scans resolve pip from PIP_CACHE_DIR first, then a fixed candidate via `pip cache dir`, falling back to the macOS default when no candidate exists. Scan output includes the source/status; `clean providers --json` includes a pip result. Invalid/outside-home roots are blocked. Candidate permissions and symlinks are checked; this is not executable authentication or an operating-system network sandbox. Tests use fixture candidates; Go discovery and full trust enrollment remain pending. The lightweight checks cannot prove a probed executable does not access the network.

### Go inventory and pip source labels

Pip source notes now identify PIP_CACHE_DIR and its path. All resolved roots must be inside the supplied home and pass ancestor/symlink validation: the earlier `/private/tmp/...` example was accepted because that temporary directory was the test's HOME, not because outside-home overrides are allowed. Fake-tool fixtures test allowed and rejected roots.

Go build and module caches are resolved separately from GOCACHE/GOMODCACHE, a fixed-candidate `go env` probe, or unverified defaults. Each scan row reports its source; modules remain review-only and unselected. Configured scans use the same resolution for enabled Go rules. Fake Go fixtures cover output, override, invalid root, defaults and command failure. The lightweight executable policy and environment flags do not provide network isolation or authenticate binaries; full enrollment remains required before new mutation adapters.

### Scan readability and coverage follow-up

- Deliverable 1 implemented: compact measured summary, top 15, denied/blocked reasons, --all and --verbose, JSON summary and terminal-only progress clearing. Tests cover summary ordering, partial lower-bound marking, --all and piped stderr. The TUI retains its full output.
- Deliverables 2–4 remain pending: fallback inventory for refused tool probes; OrbStack/Application Support/Logs coverage; and not-covered/unaccounted-space reporting. Current tool-probe failures can still hide default roots. No relaxation of executable checks is implemented.

Deliverable 2 progress: refused pip/Go candidates now remain unexecuted while the corresponding default roots are offered for read-only inventory with an explicit refusal/unverified note. Environment roots remain preferred; safety checks are unchanged. Fixture coverage checks pip and Go refused-symlink fallback; additional full-scan snapshot coverage remains pending. No group-writable ownership exception has been adopted; root:admin group-writable directories would require an explicit future user decision and security design. Deliverables 3 and 4 remain unimplemented.

### Coverage follow-up (deliverables 3–5)

- Added read-only OrbStack filesystem estimates, per-directory Application Support and user Logs inventory, with review labels and opt-in config rules. Docker Desktop remains a filesystem estimate. Apple-name and protected-name exclusions apply; name-based filtering cannot reliably identify every Apple-owned or sensitive application directory. No daemon is contacted. Downloads/Documents remain opt-in.
- Added a not-covered section and home-used-minus-measured-allocated estimate; the estimate is not reclaimed storage and may be distorted by shared extents, other-volume configured roots, concurrent changes and APFS accounting. Missing capacity or a negative result is unavailable.
- Added full inventory fixtures using refused fake pip/Go candidates and measured default roots. These use assertions on report text rather than a snapshot framework.
- Validation: make test, make security and make release-check passed for each of deliverables 3–5. One earlier deliverable 4 test run hit a recovery-operation lock contention; a fresh full run passed.

### Cache versus user-data reporting

D1 implemented: class/risk-grouped allocated totals in the text summary and `grouped_totals` in scan JSON, with per-group lower-bound flags. Unknown or unattributed generic caches remain in the unknown group; review-risk caches and user state are separate from conditional caches. D2 class filtering, D3 category view, D4 available-space samples and D5 recovery-test isolation remain unimplemented. No cleanup or scanning policy changed.

### Class filtering and category reporting

D2 implemented: class labels appear in Top and --all rows; --class cache/review/all filters those rows while leaving totals unchanged. Cache and review follow the D1 class/risk groups; unknown and project-output rows remain available with --class all. Invalid values fail before scanning.

D3 implemented: --by-category reports ten categories and optional JSON categories, using an explicit provider table checked against the shipped catalog and registry. Known Brave, Firefox and Zen generic-cache owner names route to browser caches; other generic entries remain generic. Unknown category sizes stay unknown and denied Trash reports unknown. Name-based browser attribution is a heuristic, not proof of cache content. D4 available-space samples and D5 recovery test isolation remain pending. No mutation policy changed.

### Inventory gap work in progress

D1 is **not complete**. Existing descendant traversal already refuses differing device IDs and symlinks; this follow-up isolates and tests the device comparison only. Root mount detection, virtual/network filesystem detection, same-device mount/firmlink handling, explicit skipped-provider rows and full traversal fixtures are still required before claiming virtual mounts are never entered. D2 scoped project flags, D3 home dot-directory inventory, D4 Library/container breakdown and D5 access diagnostics are also not implemented. No scan policy or deletion behavior changed in this follow-up.

### Virtual mount follow-up

A default scan now emits a size-unknown Skipped row for an existing `~/OrbStack` view without opening it, including when the final component is a symlink, and marks skipped filesystems in Not covered. A fixture checks this reporting and existing device comparison. This does **not** implement the requested general guarantee: filesystem type detection, root and ancestor mount checks, same-device mounts/firmlinks, per-descendant skipped rows, and rejection of explicitly configured scopes inside virtual locations remain unimplemented. Do not claim virtual or mounted filesystems are never walked. The evidence baseline is local-only and ignored. No deletion behavior changed.

### Filesystem type guard (partial)

On macOS, inventory path validation now checks `statfs` for each existing directory component and refuses types outside the `apfs`/`hfs` allowlist. Tests cover the allowlist, including the specified FUSE/network names. This also refuses configured roots on those types. Non-macOS builds currently have no equivalent check. This is not the complete mount-safety deliverable: different-device roots on APFS/HFS, same-device mounts and APFS firmlinks are not comprehensively detected; descendants are skipped without separate rows; and injected full-traversal fixtures and accounting assertions are still missing. Do not claim all mounts are excluded. Existing inventory path checks are path-based and may race filesystem changes. No executable or deletion policy was changed.

### Scoped project output follow-up

Added a README example for the already-supported persistent project scope and a test that extracts and validates the actual example with the configuration parser. The requested `--project-root` flag and the new project artifact reporting remain unimplemented. iOS Podfile/Xcode discovery and exclusion of tracked source remain pending; existing metadata inventory does not prove a discovered output is untracked. No project mutation behavior changed. The mount-detection gaps in the filesystem-type guard section above remain unresolved.

## Narrow cleanup v1 authorization (user amendment)

The user authorizes a narrowly scoped v1 implementation, subject to the staged gates below; this is not permission to run cleanup now. Only individually reviewed, validated direct filesystem operations on playwright-browsers, puppeteer-browsers, cypress-browsers, pnpm-store, the resolved npm cache directory, gradle-cache's caches subfolder, cargo-registry cache, and verified Brave, Zen, Firefox, Chrome or Arc app-user-cache subtrees may be eligible. Entries marked user_state, unknown_content or protected remain ineligible; unverified default locations require an explicit provider-naming flag. Generic attribution does not override these exclusions.

Never delete Application Support, Containers or Group Containers, Docker/OrbStack data, simulators, Xcode data, backups, Trash, Downloads, Documents, project output, logs or any other user state. No sudo and no external cleanup commands. A best-effort read-only in-use observation may be used, but cannot establish exclusive access. Full tool trust enrollment is not needed for direct file operations; it is not bypassed for execution of external tools.

Deliver sequentially behind gates: (0) isolated fixture safety and three consecutive test runs; (1) immutable reviewed plans with versioned, atomic storage, identity and completeness checks, no mutation; (2) interactive default-No apply to same-volume recovery, revalidation, per-unit journal and no automatic replay; (3) optional permanent deletion with separate default-No irreversible confirmation, descriptor-relative staged deletion, crash/fault tests and observed available-space change; (4) documentation. No later slice ships before its gate. Moving to recovery does not free disk space. Actual availability change is an observation affected by background activity, not an exact causal byte count. Existing mount-detection and activity-evidence gaps must not be silently treated as resolved.

### Narrow v1 Slice 0 audit

**Gate not passed.** `configured_inventory_runs_in_menu_without_default_roots` starts a TUI inventory action that derives HOME from the test process; the inventory validates that real directory even with no configured rules. Other UI actions also derive HOME from the process. Recovery unit tests shown in `src/recovery/mod.rs` and `src/recovery/purge.rs` use distinct named temporary roots with per-root lock files, but the earlier intermittent lock failure has not been explained. A complete per-test fixture-home injection and an enforcement guard proving no real-home/recovery access are still required; a source-text assertion would not prove that property. Three consecutive `make test` runs have not been performed for this gate because current tests are not yet verified hermetic. No plan or apply command was added.

### Slice 0 isolation follow-up (incomplete)

TUI App now accepts an injected home path, and `configured_inventory_runs_in_menu_without_default_roots` explicitly creates and passes a disposable fixture home rather than using the process HOME. `cargo test --locked --bin optimac configured_inventory_runs_in_menu_without_default_roots` passed (1 test). The production constructor still derives HOME normally; this change does not use process-global environment mutation.

The Slice 0 gate is **not passed**: there is no comprehensive enforcement guard against real-home/recovery or outside-fixture I/O, remaining filesystem tests have not all been converted to unique managed fixture homes, and a concurrent recovery-lock regression test is absent. `operation_lock` is scoped to each recovery root's `.lock` and uses nonblocking flock; inspected recovery tests use differently named PID-based roots, but the previous contention cause remains unproven. No sleeps or retries were added. The complete suite was not run because the guard is not yet verified. No cleanup functionality was added.

### Shared filesystem path context

Added `AppPaths` with explicit home input and derived recovery/config paths. CLI home resolution and library action entry points use the shared resolver; recovery derives its existing path through the same context. TUI actions receive their home at construction and pass it into library actions. In TUI unit tests the default constructor has no home, so tests must inject one and missing input fails closed; no concurrent test changes process-global HOME. CLI fixture tests set HOME only on their child process.

Focused tests passed: `configured_inventory_runs_in_menu_without_default_roots`, `missing_fixture_home_fails_closed`, `injected_paths_resolve_consistently_and_missing_home_fails_closed`, and `fixture_paths_are_used_by_cli_history`. The full suite was not run. This is dependency injection, not filesystem authorization: existing APIs still accept paths, other tests need conversion and an enforced fixture boundary, symlink/race protections must be audited, and the recovery lock contention remains unproven. Slice 0 remains incomplete; no planning or deletion changes were added.

### Fixture path capability groundwork (Slice 0B incomplete)

Added `FixtureRoot`, an explicit descriptor-held private temporary-directory capability with no-follow inspection of existing descendants; its focused fixture test rejects traversal, symlink escapes, and outside paths without modifying targets. `cargo test --locked fixture_capability_rejects_escape_and_symlink_without_mutation` and `cargo test --locked --bin optimac configured_inventory_runs_in_menu_without_default_roots` passed. The full suite was not run.

**No recovery mutation entry point is protected by this capability yet.** Recovery root creation, journal and lock creation, move/restore and purge remain unprotected by fixture authorization, including in separately executed CLI processes. The capability only opens existing paths for inspection; it does not authorize, create, rename or delete anything and its temporary-directory location is not an OS sandbox. Missing authorization does not yet fail closed in recovery. Integration, descriptor-relative authorization for each mutation, separate CLI test capability transport without a production bypass, remaining tests and recovery-lock investigation are still required. Slice 0 remains incomplete.

### Fixture-scoped recovery initialization (Slice 0B.1)

Added `recovery::initialize_fixture_recovery`, which requires an explicit `FixtureRoot` capability and creates the recovery directory with `mkdirat` relative to its pinned fixture directory descriptor. The capability uses fixed OS temporary roots, no-follow component traversal, ownership and permission checks; the operation rejects missing authorization, other homes and traversal, and refuses a symlink recovery directory. A successfully returned descriptor pins the opened recovery directory. Focused `fixture_recovery_initialization_requires_authorization`, `fixture_capability_rejects_escape_and_symlink_without_mutation`, and TUI inventory tests passed. No full suite was run.

This is a separate fixture-scoped entry point; **existing production and test recovery_root, execute, journal and lock creation, restore, inspect and purge paths do not require this capability**. CLI integration does not transport it. It is not a universal filesystem guard and does not solve movement of a fixture directory by another process with the same user privileges after opening; no claim of full recovery isolation is made. Slice 0B remains incomplete and the recovery-lock contention investigation remains open.
