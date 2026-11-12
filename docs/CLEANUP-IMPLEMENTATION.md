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

### Shared recovery metadata context (Slice 0B.2)

Implemented `RecoveryContext` with explicit `RecoveryPolicy::Production` or `RecoveryPolicy::Fixture(Option<&FixtureRoot>)`. Fixture initialization requires the existing capability; missing authorization fails before writes, without environment-based fallback. The context retains the opened recovery directory and policy. `initialize_fixture_recovery` now delegates to this context. Existing `recovery_root` uses the same initializer with the original production path, ownership and permission validation preserved.

The existing `execute_with_progress` now uses context locking and journal creation, replacing its inline journal-writing implementation. Both fixture and production contexts use the same nonblocking flock and version-1 record serialization. Lock opening uses `openat` with no-follow and the existing regular-file, owner and single-link checks. Journal directory creation uses `mkdirat`; record creation uses exclusive no-follow `openat`, mode 0600, and file/directory syncs relative to pinned descriptors. Journal IDs reject traversal and paths. Legacy `operation_lock` delegates to the shared descriptor lock implementation. No payload move, restore or purge authorization migration was made.

Focused validation passed: `cargo test --locked fixture_recovery` (2 tests), `cargo test --locked recovery_lock` (1 test), `cargo test --locked recovery_journal` (2 tests), and `cargo test --locked --bin optimac configured_inventory_runs_in_menu_without_default_roots` (1 test). Fixtures cover missing capability, outside/traversal/symlink rejection, unchanged rejected targets, lock symlink/hard-link rejection, lock exclusivity, journal format and restrictive permissions, production permission rejection, and replacement of the recovery path with a symlink after initialization: subsequent metadata writes stay in the pinned directory. All filesystem targets in these tests are disposable temporary fixtures. The full suite was not run.

**Slice 0 remains incomplete.** Explicit fixture context metadata operations are authorized; production policy remains explicitly available and existing high-level operations still choose it. Existing tests and CLI integration must opt into fixture authorization; this is not automatic test detection or complete fixture-only enforcement. Move, restore, purge journals/deletion, other filesystem writes and reads remain outside this boundary. The context cannot prevent another process with the same user privileges from relocating an opened fixture or replacing entries inside a private directory; it is not an OS sandbox. Production root creation retains its existing path-based behavior. CLI capability transport, remaining test migration, complete isolation validation, recovery-lock contention investigation and three full-suite runs remain pending. No Slice 1 functionality was added.

### Explicit high-level recovery authorization (Slice 0B.4)

`execute`, `execute_with_progress`, `restore`, and purge `plan`, `execute`, and `inspect` now require an explicit `RecoveryPolicy`; internal root initialization and payload checking propagate that policy instead of selecting Production. The existing CLI and production TUI explicitly select production authority. Injected TUI homes default to `Fixture(None)`, which fails closed. Fixture metadata initialization, locking and journaling continue through the shared descriptor-based context. Fixture payload/reconciliation calls are rejected before metadata writes: move/restore/purge authorization is deliberately not implemented in this milestone.

Added the compile-time Cargo feature `fixture-only`, enforced in the shared authorization check in both library and separately executed CLI binaries. In that build even an explicit `Production` value is refused, with no environment variable or runtime configuration override. This is the smallest CLI boundary change: integration tests must build and launch the fixture-only binary; changing child HOME alone is not authorization. A subprocess regression runs `recovery inspect 123` against an empty disposable home, verifies the production-authority error and verifies no files were created. The normal production build remains enabled, with existing validation unchanged.

Final focused results: `cargo test --locked fixture_recovery` passed 4 tests; `cargo test --locked recovery_lock` passed 1; `cargo test --locked recovery_journal` passed 2; `cargo test --locked --bin optimac configured_inventory_runs_in_menu_without_default_roots` passed 1; `cargo test --locked --features fixture-only fixture_recovery` passed 6 (5 library, 1 CLI subprocess). The production journal regression also runs an empty high-level recovery execution on a disposable fixture, without external commands or payload changes. An initial compile attempt found two legacy progress-test call sites missing the new argument; both were updated before the passing validations. No full suite or real-user-data mutation was run.

**Slice 0 remains incomplete.** A normal build still exposes explicit production authority, including to library tests; the compile-time fixture-only boundary must be selected for fixture integration execution. Legacy payload tests now explicitly name their existing production policy to preserve their assertions and compile, but were not executed or migrated; they will fail closed under fixture-only until payload authorization is implemented. This feature does not intercept unrelated filesystem access or reads such as history/inventory, and no full-suite isolation claim is made. Fixture payload authorization, test migration/build-runner enforcement, CLI capability transport for positive fixture operations, lock-contention investigation, and the full-suite gate remain pending.

### Fixture-only focused test runner (Slice 0B.5)

Added `scripts/test-safe.sh`; `make test` and both CI workflows now use this fixture-only focused set, not the full suite. It runs the four requested filters plus a CLI executable rejection regression, all with `--features fixture-only`. CLI integration tests refuse to compile without that feature and use a common launcher which checks the actual Cargo-provided executable's fixture-only probe before each invocation. The subprocess recovery regression confirms the probed CLI rejects production authorization and makes no recovery files. The probe verifies the expected build configuration, not the identity of an adversarial executable; code bypassing the launcher is not intercepted. A test checks missing and non-executable CLI paths are refused.

The first fixture recovery run failed: two fixtures in one process shared the same timestamp-based path. Added an atomic per-process sequence to their names. The subsequent `./scripts/test-safe.sh` run passed: fixture_recovery 6 tests (5 library, 1 CLI), recovery_lock 1, recovery_journal 1, and TUI inventory 1. The additional CLI launcher regression passed 1. The normal-build production journal regression is excluded from fixture-only builds, and was not rerun here.

This is **not full-suite validation**. Ordinary `cargo test --lib` can still build production authority; existing legacy tests, unrelated filesystem reads/writes, arbitrary processes and tests bypassing the shared launcher are not sandboxed. The runner deliberately covers a narrow audited set; CI's green result must not be read as a complete test-suite pass. Migration, fixture payload authorization, broad read/write isolation and lock-contention investigation remain open. Slice 0 remains incomplete.

### Fixture payload moves with bounded threat model (Slice 0B.6)

The user clarified that Slice 0 fixture isolation addresses accidental out-of-fixture access, not a malicious concurrent process with the same user privileges; production cleanup safety is unchanged. Added `RecoveryContext::move_fixture_payload`, requiring the same explicit `FixtureRoot` identity that initialized the context, a matching existing journal record, and a single-link user-owned regular-file candidate. Source parents are opened beneath the fixture with no-follow component traversal; the destination is the opened recovery directory plus a numeric/hyphen journal ID. The source is opened no-follow and its recorded device, inode, size and mtime are rechecked immediately before a descriptor-relative `renameatx_np` with `RENAME_EXCL`. A device mismatch is refused and there is no copy fallback; non-macOS builds reject this operation. No production move, restore or purge behavior changed.

Tests use private disposable fixture roots. Three focused macOS tests passed for successful content-preserving move, occupied destination, missing/mismatched authorization, outside source, traversal, symlinked source/parent/destination, changed source and unchanged outside data. `./scripts/test-safe.sh` passed (fixture_recovery 9 tests including the CLI subprocess; lock, journal, TUI and executable guard 1 each). Each of the three `fixture_recovery_payload_move_*` tests also passed separately (1 each); `cargo test --locked --features fixture-only fixture_recovery` passed 9. No real cross-device mount was exercised; rejection is implemented by comparing source and destination device IDs, with the atomic rename also returning an error on cross-device attempts. No full suite was run.

Limits: pre-move identity checking is not atomic with rename; a same-user concurrent actor can replace the source entry or relocate pinned directories. Descriptor pinning avoids following replaced path components, but does not prevent relocation of an opened directory. A directory-sync failure after the rename can return an error after the payload has moved. The new method is an explicit fixture-context primitive; high-level `execute_with_progress` still refuses fixture payload authority, and fixture restore, purge, test migration and CLI capability transport remain pending. Slice 0 is incomplete.

### Independent read-only planning (Slice 1, partial)

The user authorized a read-only planning proposal independently of the unfinished Slice 0 gate; apply and permanent cleanup remain blocked by their original gates. Added `clean plan [--config PATH] [--deep] [--json]`. It consumes the same fresh inventory and config scopes/exclusions as scan, groups provider-root estimates by existing category, and reports class, risk, status, path, estimated allocated bytes and an explanation. Missing sizes remain unknown. Complete conditional-cache rows in the narrow provider list may contribute to the candidate estimate; denied, partial, blocked, unverified, unknown, review-risk, out-of-scope and overlapping rows do not. The current npm root is fixed rather than verified by a resolver, so it is excluded. The output warns that estimates do not guarantee available space freed and do not authorize cleanup. It does not invoke recovery mutation or write a plan file.

This is an aggregate inventory proposal, not the earlier full immutable Slice 1 plan: it does not enumerate individual target units, capture identities, produce an ID/hash, save a versioned plan, implement provider selection, or establish deletion safety. Existing inventory budgets, root/source verification limits, protected-path and mount-detection gaps still apply; a zero candidate estimate is not proof there is nothing to clean. User-state and review entries remain review-only. No apply or permanent-cleanup implementation was added or enabled.

Focused fixture-only validation: `cargo test --locked --features fixture-only clean_plan` passed 2 tests; each of `clean_plan_groups_and_excludes_unknown_denied_partial_unverified_and_overlaps` and `clean_plan_cli_respects_config_exclusions_and_keeps_files_unchanged` passed individually (1 each). The CLI fixture tests both text and JSON, confirms excluded data does not contribute to logical size, and checks fixture contents remain unchanged. `./scripts/test-safe.sh` passed including the new planning tests. The unsafe legacy full suite was not run. Slice 0 remains incomplete: full fixture-only isolation, high-level fixture payload authorization, restore/purge isolation, test migration and lock-contention investigation remain open. No later mutation gate is waived.

### Slice 0C: CLI fixture migration (partial)

Inspected 13 targeted filesystem tests in detail: eight CLI integration fixtures and five legacy recovery tests. Migrated all eight filesystem-using tests in `tests/cli.rs` to create a unique mode-0700 temporary home and explicitly open it with the existing `FixtureRoot`; the counter plus process ID and timestamp avoid collisions between concurrently created homes. Commands that scan files now pass that home only to the child process; no test mutates process-global HOME. Existing assertions are retained. This does not make the child process an OS sandbox: holding a `FixtureRoot` does not enforce all child-process reads or writes, and the fixture-only build guards recovery production authority rather than arbitrary filesystem operations.

Each migrated test passed individually with `cargo test --locked --features fixture-only <name>`: `fixture_cli_rejects_missing_executable`, `analysis_and_duplicates_use_disposable_fixtures`, `artifact_sizes_sum_scanned_files`, `cleanup_inventory_json_is_read_only_and_reports_unknown_roots`, `configured_inventory_applies_scopes_and_exclusions`, `fixture_paths_are_used_by_cli_history`, `fixture_recovery_cli_cannot_select_production_policy`, and `clean_plan_cli_respects_config_exclusions_and_keeps_files_unchanged` (1 test each). `./scripts/test-safe.sh` passed, and `cargo test --locked --features fixture-only --no-run` compiled the library, CLI and integration targets; this is not a full-suite execution or safety proof.

Four legacy recovery tests are blocked without the still-unimplemented high-level fixture move/restore/purge paths: `personal_selection_moves_only_reviewed_file_and_restores`, `app_bundle_roundtrip_and_changed_candidate_refusal`, `recover_without_overwriting`, and `purge_removes_only_reviewed_recovery_payload`. Their assertions were not weakened or skipped. Other library and TUI filesystem tests still use PID-named or otherwise unmanaged temporary paths, and some non-filesystem-specific tests launch subprocesses with inherited environment; the initial search is not a complete execution-path audit. Ordinary library test builds can select production recovery policy; arbitrary filesystem reads/writes and CLI subprocesses are not comprehensively fixture-authorized. The prior lock contention remains unexplained. Slice 0 remains incomplete: classify and migrate the remaining tests, implement authorized fixture restore/purge separately if approved, investigate lock isolation, and only then run the complete suite three times.

### Slice 0C.2 scorecard: library and TUI migration (partial)

The fixture-only macOS test harness lists 89 tests: 61 library, 16 binary/TUI and 12 CLI integration. This scorecard covers 24 deliberately inspected tests; **65 remain unclassified**, not presumed safe. The categories are mutually exclusive for this scorecard: 8 previously migrated fixture-based CLI tests; 5 tests needing fixture migration; 4 blocked legacy recovery tests; 5 pure logic tests (`broad_groups`, `readable_units_preserve_boundaries`, `filesystem_allowlist_is_fail_closed`, `device_boundary_comparison_does_not_authorize_crossing`, `developer_environment_roots_are_explicit_and_safe`); and 2 unverified subprocess tests (`diagnostics_use_readable_headings_and_report_failures`, `subprocess_cancellation_and_failure_are_reported`). “Isolated” here means explicitly fixture-based within the bounded accidental-access threat model; it is not an OS-enforced sandbox.

| Metric | Before | After |
|---|---:|---:|
| Classified tests | 24 | 24 |
| Isolated tests | 8 | 13 |
| Blocked tests | 4 | 4 |
| Unclassified tests | 65 | 65 |

Added a test-only/fixture-only constructor to the existing `FixtureRoot` that creates unique private mode-0700 roots under fixed OS temporary locations. Migrated `counts_metadata_without_following_links_and_honors_limits`, `byte_comparison_is_exact_and_cancellable`, `injected_paths_resolve_consistently_and_missing_home_fails_closed`, `menu_navigation_and_background_actions`, and `configured_inventory_runs_in_menu_without_default_roots`; the TUI menu test now injects its fixture home through `AppPaths`. Assertions were preserved. Each named test passed individually (1 each) under `cargo test --locked --features fixture-only` (TUI tests additionally used `--bin optimac`). The expanded `./scripts/test-safe.sh` passed, and `cargo test --locked --features fixture-only --no-run` compiled all three targets. No full suite ran.

The four blocked tests still call high-level production-policy payload/restore/purge paths; fixture-only rejects those calls. A previous narrow fixture-context move primitive was implemented, but it cannot replace high-level restore/purge coverage without changing the behavior being tested. Two alternatives: (1) extend the existing capability and context through high-level fixture move, restore and purge, then migrate the original tests; this maintains production-like coverage but needs careful descriptor-relative mutation work and fault tests; (2) run the original production-policy tests in a dedicated disposable macOS VM or user with isolated home and filesystem permissions; this preserves the production code path but adds infrastructure and requires proving the host boundary. A pure planning/backend split can provide fast non-destructive coverage but cannot replace either mutation path's tests. The safest next small milestone is to classify and migrate the remaining independent read-only fixture tests, then design the fixture restore boundary separately; do not replace or ignore the blocked assertions.

Remaining risks: the feature does not intercept arbitrary filesystem or subprocess access, fixture descriptors are not an OS sandbox, test cleanup still uses paths inside fixtures, and ordinary library builds can select production recovery authority. The unclassified tests, unverified subprocesses, CLI capability transport, fixture restore/purge, unexplained lock contention and three full-suite passes remain open. Slice 0 remains incomplete.

### Process-isolation deliverable A: enforced complete manifest

Generated `docs/TEST-CLASSIFICATION.json` from the compiled fixture-only macOS harnesses' `--list` output: 89 tests, comprising 5 pure, 80 fixture_fs (unreviewed), 4 recovery_blocked and 0 subprocess_audited. Uncertain cases are explicitly unreviewed, not certified safe. `make test-classification` rejects missing, stale, duplicate or unknown-class entries; `make test` depends on it. Validation passed: live classification check (89 names), four checker cases (missing/stale/duplicate/matching), `make security`, `make release-check`, and diff whitespace check. Listing executes no test bodies.

Deliverable B is not implemented and the full suite has not run. There is a requirements conflict: comparing the real recovery directory's listing/mtime requires reading its metadata, while the hard rule forbids tests reading the real home/recovery directory. No such read was performed. Also, clearing the environment and replacing HOME/TMPDIR cannot deny absolute-path access or native home lookups by subprocesses; it is not the requested process-level access boundary. Completing B requires choosing an OS-enforced sandbox/isolated execution environment and clarifying whether an external observer may read only the real recovery directory metadata. Until that boundary is implemented and verified, the runner remains focused. C (recovery tests), D (fake executable subprocess migration), and E (parallel homes and three full runs) remain undone. No tests were ignored or production protections changed. Slice 0 remains incomplete.

### OS sandbox B1: full-suite runner

Added `scripts/test-sandboxed.py`; `make test` now builds first and selects all test harnesses under sandbox-exec, with the former subset retained only as `make test-fast`. The generated profile denies writes except the fresh private temporary tree and /dev/null and /dev/tty, denies network, and denies real-home file-read-data except compiled target/debug artifacts needed by the harness and CLI child. Runtime HOME/TMPDIR/XDG directories are isolated and the inherited runtime environment is cleared. Cargo's build environment is unchanged. Unexpected build artifact paths and sandbox failures fail closed. macOS 26.6.2 permits sandbox-exec outside the agent sandbox; nested sandbox_apply fails with Operation not permitted. Sandboxed listing found all 89 tests. B2 canary/metadata proof and full-suite runtime results remain pending; this B1 validation ran no test bodies.

### OS sandbox B2: canary and metadata observer

The outer harness compiles a tiny canary, then runs it under the identical profile: create-new attempts in real home and real recovery both returned PermissionDenied, while a temporary write succeeded. The outer observer recursively compares lstat mode/size/mtime and names without following symlinks or reading file contents, including when tests fail. First full execution: 89 tests, 78 passed and 11 failed (library 52/61, TUI 14/16, CLI 12/12). Real recovery metadata was unchanged across 9,884 entries. Five failures came from the fixture constructor writing directly under /private/tmp outside the sandbox tree, two from fake tool ancestry under world-writable /private/tmp, and four from fixture-only rejection of production recovery authority. These are C follow-ups, not a green suite. Canary proof passed; no real-home write succeeded.

### OS sandbox C: full-suite fixes and dedicated recovery target

Fixed the test-only FixtureRoot constructor to honor TMPDIR instead of writing directly under /private/tmp. The sandbox now uses the system temporary parent so fake pip/Go binaries have acceptable non-world-writable ancestors; executable policy is unchanged. All normal harnesses passed: 85 passed, 4 ignored, 0 failed. The ignored tests are `personal_selection_moves_only_reviewed_file_and_restores`, `app_bundle_roundtrip_and_changed_candidate_refusal`, `recover_without_overwriting`, and `purge_removes_only_reviewed_recovery_payload`. Their assertions are unchanged. `make test-recovery-isolated` builds the non-fixture-only library outside the sandbox and runs only these ignored tests inside the same sandbox: all 4 passed. Both runs passed the canary and left all 9,884 observed recovery metadata entries unchanged. Manifest fixture_fs entries now record a sandboxed pass, not manual certification. Production mutation code is unchanged. D and E remain pending.

### OS sandbox D: diagnostic subprocess fixtures

The two diagnostic subprocess tests now invoke only an absolute fake executable created inside their private fixture. Its interpreter uses shell builtins only; success/failure/cancellation assertions remain, it rejects inherited HOME/TMPDIR/XDG_CONFIG_HOME, and both tests assert a relative executable is refused rather than searched through PATH. No real diagnostic utility is invoked by these two tests. Updated their manifest class to subprocess_audited. Full sandbox validation passed: 85 passed, 4 ignored, with canary success and unchanged real recovery metadata. Counts now: 78 fixture_fs, 5 pure, 2 subprocess_audited, 4 recovery_blocked.

### OS sandbox E: contention and stability results

Added `make test-concurrent-isolated`: two concurrently started sandbox runners execute the recovery-lock test with separate temporary homes. Both passed; their recovery paths and device/inode lock identities differ. The test holds its lock for 200 ms to exercise overlapping workers, still checks same-root exclusivity, then releases and reacquires it. This is not a retry or a workaround for a failing lock. The earlier historical contention was not reproduced or causally diagnosed; distinct sandbox trees now prevent accidental shared test recovery paths.

Three consecutive `make test` runs passed on macOS 26.6.2: each listed 89 tests and ran 85 successfully, with 4 explicitly ignored production-policy tests and 0 failures (library 57 passed/4 ignored, TUI 16 passed, CLI 12 passed). All three also passed formatting and clippy. Each canary denied both forbidden writes and allowed the temporary write; each outer metadata comparison reported 9,884 unchanged recovery entries. The separate recovery-only sandbox run passed all four ignored tests. Manifest counts are 78 fixture_fs, 5 pure, 2 subprocess_audited, 4 recovery_blocked. `make security` and `make release-check` passed after each milestone.

The process-level Slice 0 safety-net deliverables B1–E are now validated on this machine, with the explicitly permitted four-test separate-target arrangement. This supersedes the former subset-only gate: ordinary Cargo invocations remain unsandboxed, so use the make targets. The OS profile permits compiled target/debug artifact reads inside real home and device writes only to /dev/null and /dev/tty; build tools and the metadata observer run outside it. This does not certify production cleanup race handling, prove the historical contention cause, or authorize new cleanup functionality. No new planning/apply/deletion behavior was introduced.

### Immutable planning Slice 1: authorization amendment

Slice 0's OS sandbox gate is complete at 70a7f5b. The spec and adapter contract now explicitly authorize plan creation only for narrow cleanup v1. This does not implement immutable plans or enable apply/permanent deletion. Implementation will reuse inventory and preserve existing classifications; unknown-content browser rows remain ineligible under the explicit exclusion rule. Model/storage, exact-unit builder, CLI selection/show/list and their sandboxed validation remain pending.

### Immutable planning: model and storage

Added `cleanup::plan_store`: strict schema-v1 content/identity types, deterministic SHA-256 IDs and checksum verification, top-level target shape checks, and private app-data storage. Storage uses no-follow descriptor-relative opens, mode-0700 app directories, mode-0600 files, fsync and exclusive atomic publication. Existing plans are never overwritten. Load/list do not create directories; corrupt/future-schema/unsafe files fail closed. Creation refuses overlap with caller-supplied scan roots or recovery. Failed publication may leave a private `.tmp` file, which list ignores; no automatic deletion is introduced. The digest detects corruption, not a malicious same-user rewrite or authorization to apply.

Validation: `make test` passed 87 tests, 4 ignored (91 listed), including 2 new model/storage tests; `make test-recovery-isolated` passed all 4; `make security` and `make release-check` passed. Sandbox canaries passed and real recovery metadata remained unchanged. This library is not yet connected to CLI planning. Exact-unit collection, provider selection, exclusions, CLI show/list and requested end-to-end fixtures remain pending. Apply/permanent cleanup remain blocked.
