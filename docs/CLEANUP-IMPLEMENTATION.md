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
