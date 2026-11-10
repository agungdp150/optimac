# Current implementation

The menu now enables conservative developer-cache cleanup, owned app-bundle removal
and Rust recovery restoration after exact-plan review and y/n confirmation.
Filesystem moves use descriptor-relative exclusive rename on macOS, synced intent
records, a recovery lock, identity checks and active-file detection. Files in use
or with unknown status are skipped. No permanent deletion or copy/delete fallback.
Only old regular files in explicit developer caches qualify; app removal preserves
all personal/support data. Restore never overwrites. Tests cover real disposable
moves, restore collisions, app roundtrip, changed identities and confirmation gates.
Historical entries below describe earlier stages and are superseded by this section.
CLI destructive flags, legacy restore import, browser cleanup and service toggles
remain unsupported. Memory optimization is diagnostic only. This is conservative
menu coverage, not full parity with every former Go behavior.

# Recovery layer in progress

Conservative cleanup preview now selects only owned, single-link regular files
in five developer-cache roots, older than 30 days. Symlinks and unsafe ancestor
paths are rejected. The internal recovery prototype writes and syncs per-item
intent records before exclusive same-volume rename and refuses restore collisions.
Fixture tests cover restoration, destination collisions, recent files and symlinks.
It is deliberately not callable through cleanup execution or restore commands yet:
path replacement races and active-file checks remain release blockers. `log` and
`history` can inspect Rust recovery IDs without changing data. Legacy records are
not imported. No user data was moved during implementation.

# Read-only feature update

Menu/CLI jobs now cover large files, byte-exact duplicate comparisons, artifact
candidates, Homebrew updates, launch-item inventory, legacy threat-name hints,
disk/snapshot diagnostics, manual support-folder review, macOS security posture,
app inventory, memory and system diagnostics. Cleanup offers a conservative preview.
Jobs run in the background and results scroll. No mutation engine has been added;
cleanup execution, uninstall and durable restoration remain unfinished. The former
announcement of a complete migration referred only to source/build language, not
feature parity. Historical notes follow.

# Current status: Rust-only cutover

The Go implementation has been removed at the user’s request. Cargo now lives at
the repository root and produces `optimac`. Default build, test, install, CI and
Homebrew HEAD paths use Rust. This is a breaking feature reduction: only metadata
inspection and the responsive UI are available. All legacy commands fail explicitly.
Full command parity, recovery compatibility and safe cleanup remain future work.
The following audit and implementation notes describe the historical migration.

# Rust migration and optimization audit

Status: migration started; Go remains the shipping implementation. The Rust binary
is a read-only prototype, not a replacement. No production cleanup was executed
during this audit. Device hangs have not been reproduced or profiled.

## Findings from source inspection

| Priority | Evidence | Required change |
| --- | --- | --- |
| P0 | `internal/opti/clean.go`, `browser.go`: trash initialization failure sets `useTrash=false` | Fail closed; never implicitly choose permanent deletion. Audit all mutation callers for the same pattern. |
| P0 | `trash.go`: moves precede `Commit`; log uses direct `WriteFile`; callers can ignore commit errors | Durable per-item intent/completion journal, atomic updates, locking, interrupted-operation recovery. |
| P0 | `trash.go`: any rename failure triggers copy/remove, not only cross-device errors | Preserve the original error; initially refuse cross-volume mutation. Later use a verified, metadata-preserving copy protocol with recovery. |
| P0 | `safety.go`: lexical prefix checks, followed by path-based operations | Handle symlink ancestors and path replacement races with descriptor-relative operations and identity revalidation. Canonicalization alone is insufficient. |
| P0 | `clean.go`: broad cache/temp rules, Xcode Archives and Saved Application State | Replace automatic discovery-to-deletion with reviewed rule manifests. Archives and state are not ordinary disposable caches. |
| P1 | `browser.go`: history/cookies and caches share execution; no running-app gate in this path | Separate explicit privacy actions from optimization; skip running or unknown-state applications. |
| P1 | `process.go`: detection errors return false | Represent running, stopped, and unknown separately. Unknown must not authorize mutation. |
| P1 | `optimize.go`: unconditional sync then purge, no timeout in shared command runner | Retire default RAM purge; use pressure diagnostics. Add bounded subprocess execution and cancellation. |
| P1 | `sizes.go`: 4–16 parallel recursive walks; errors become zero sizes | Start with low concurrency, expose incomplete results, add work budgets and progress. |
| P1 | `tui/layout.go`: minimum width 30, minimum result rows 8; rune-based truncation | Layout against actual drawable cells; compact/tiny modes; grapheme-aware width. |
| P1 | `tui/tui.go`: busy state rejects most input, Ctrl-C quits without an explicit worker cancellation protocol | Cancel jobs and stop scheduling mutations; handle partial completion and terminal restoration. |
| P1 | `duplicates.go`: content matches and filename similarities coexist | Never treat name similarity as proof that deletion is safe; recheck content identity before any selected removal. |
| P2 | `path.go`: walk errors ignored; `trash.go` reports logical lengths | Track skipped/error totals and separate logical, allocated, moved, and actually reclaimed bytes. |

Likely contributors to sluggishness include concurrent metadata walks, cache
rebuilding after cleanup, copy-based trash movement, and sync/purge. These are
hypotheses until measured; Rust does not automatically fix filesystem load.

## Implementation sequence and completion gates

1. Inventory CLI aliases, exit codes, config, JSON output, and operation-log format.
   Cover scan, clean, analyze, duplicates, optimize, status, artifacts, updates,
   apps/uninstall, browser, login, threats, space, orphans, doctor, restore, history,
   trash, config and UI. Record each as port, redesign, or retire. Preserve existing
   recovery data; never silently reinterpret it.
2. Build a read-only scanner and worker/event interface. Add mount boundaries,
   cloud-placeholder detection, exclusion rules, bounded errors/progress, Ctrl-C
   cancellation, and deadlines. Do not read file contents for size estimates.
3. Implement the Rust TUI around the shared engine. Render using current frame
   size, preserve selection, clamp scrolling, and redraw on resize. No filesystem
   work on the UI thread. Test every screen at 20x5, 40x10, 80x24, 120x40 and repeated
   shrink/grow sequences, including Unicode and long paths.
4. Implement a policy engine producing immutable review plans: exact targets,
   reason, risk, size semantics, app state and identity. User-selected personal
   files are distinct from automatic cache rules. Exclusions apply to descendants.
   Reject elevated UI execution; privileged work needs a separate narrow design.
5. Implement journaled trash/restore after fault-injection tests pass. Revalidate
   the selected plan before mutation. No silent fallback, implicit permanent
   deletion, overlapping targets, or restore overwrites. Test full disks, denied
   permissions, symlink swaps, changed files, interrupted writes, concurrent runs,
   and journal corruption in disposable fixtures or an isolated macOS VM.
6. Port the remaining commands; replace RAM purge with diagnostics. Validate on
   Apple Silicon and Intel macOS. Update Makefile, release/security scripts,
   both Homebrew formula locations, CI, installer documentation and release notes.
   Remove Go only once coverage, compatibility and safety gates pass.

## Measurement protocol

Use identical generated trees for both implementations: many small files, deep
trees, wide directories, sparse files, symlinks, unreadable entries and changing
files. Record hardware, OS, fixture size, cold/warm cache conditions, wall time,
peak RSS, CPU, metadata operations, disk throughput, cancellation latency and
keyboard-to-frame latency. Never run purge to manufacture a cold-cache benchmark
on the user's working Mac. Measure idle and busy UI separately.

Initial UI target: p95 input-to-frame under 100 ms during local fixture scans.
Cancellation should stop new work within 250 ms; in-flight OS calls may block and
must be reported honestly. Release size/RSS targets require the Go baseline first.
Report measured wins and regressions; do not claim Rust is lighter without data.

## First Rust slice

`rust/` contains a dependency-free metadata scanner and `optimac-next inspect`.
It uses one worker, at most 32 open directory iterators and 100000 visited entries.
It skips observed symlinks, reports errors/partial results, and exposes cooperative
cancellation to library callers. It does not clean, optimize RAM, invoke sudo,
or replace the installed CLI.

Current limitations: no mount/cloud policy, no race-proof traversal, no TUI,
no signal-to-cancellation wiring, no time budget, and no allocated-space accounting.
Hard links are counted per pathname. Use local disposable fixtures only until
these policies are implemented. This scanner is not a deletion safety boundary.

Validation commands:

```sh
cargo test --manifest-path rust/Cargo.toml --offline
cargo clippy --manifest-path rust/Cargo.toml --offline -- -D warnings
cargo run --manifest-path rust/Cargo.toml --offline -- --help
```

## Implementation update

Implemented Rust `ui <directory>` using Ratatui/Crossterm, background scanning,
compact rendering, resize tests down to zero-sized buffers, progress snapshots,
Ctrl-C for CLI scanning, and UI cancellation. Added a 30-second cooperative scan
budget, same-device traversal, macOS dataless-entry skipping and conservative
name exclusions. OS calls in flight can exceed the cooperative time budget.
Root paths are explicitly supplied; network-root detection, ancestor-symlink
hardening and complete cloud-provider coverage remain outstanding.

Go interim fixes: trash-init failure now aborts clean/browser/artifacts/uninstall;
commit errors propagate; scan concurrency capped at two; RAM purge disabled;
archives, saved state and broad var/folders targets removed from defaults.
Earlier findings above remain the audit baseline, not a claim all are unresolved.
Durable journals, race-safe mutation, full command parity and production Rust
cutover are still required. The preview does not replace the shipping binary.
