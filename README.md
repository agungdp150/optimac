# OptiMac

OptiMac 0.2.0 is a Rust-only, read-only macOS filesystem inspector with a responsive
terminal UI. The former cleanup suite is not yet ported. No Go toolchain is needed.

## Build and run

Install a current stable Rust toolchain (Rust 1.93 or newer), then:

```sh
make build
./bin/optimac
./bin/optimac ui ./src
./bin/optimac inspect ./src
./bin/optimac version
```

`make build` builds Rust and copies the binary to `bin/optimac`. `make install`
installs it to `~/.local/bin/optimac`. An existing installed binary is not replaced
until you run that install command. `make rust-build` remains an alias.

The cat-themed home menu opens with `optimac`, `optimac ui` or `optimac menu`.
Use arrows or j/k to navigate, Enter to select, and Esc to return. The inspection
action starts a scan of the current directory (or the path supplied to `ui`).
Menu actions open bounded background jobs; mutation features remain explicitly unavailable.
Resize freely; press `c` to cancel scanning and `q` or Ctrl-C to quit.
CLI scans also support Ctrl-C. Run `optimac --help` for usage.

## Scope and limits

Scans inspect metadata on one worker, with a 100000-entry budget, a 32-directory
stack limit and a cooperative 30-second deadline. Observed symlinks, filesystem
boundaries, selected sensitive/cloud directories and macOS dataless entries are
skipped. Filesystem calls already in flight can exceed the deadline. Root paths
are explicit; use local directories. Ancestor symlinks, network roots and all
cloud providers are not comprehensively handled yet.

Sizes are logical lengths, not allocated or reclaimable disk space. Hard links
are counted per path. Output reports errors, skipped entries and budget limits.
Exit codes: 0 command completed (review diagnostic output for partial results),
1 top-level error or cancellation, 2 invalid or unavailable command. `inspect` also
returns 1 for traversal errors or budget limits. Other diagnostic reports may
contain per-check failures or partial results while returning 0. No files are cleaned or optimized.

## Migration from 0.1.x

This is a breaking Rust-only cutover. The menu and CLI now support:

- `inspect`, `ui` / `menu`, help and version.
- `analyze [directory]`, `duplicates [directory]`, `artifacts [directory]`.
- `clean` (conservative cache preview only), `apps` (inventory only).
- `updates`, `login`, `threats`, `space`, `doctor`, `status`.
- `orphans` (manual support-folder review, not proven orphan detection).
- `optimize` (memory diagnostics, no purge).
- `log` / `history` (Rust recovery-record inventory only).

All current actions are read-only. Cleanup execution, uninstall, restore, browser
cleanup and login-item toggles remain unavailable. The menu labels distinguish
preview/inventory from execution. Results support arrow-key scrolling and `c`
cancels work. External diagnostics have a 15-second timeout per command and bounded
captured output. Duplicate comparisons have separate time and comparison budgets.
Filename threat checks are legacy hints, not antivirus verdicts.

Existing configuration and `~/.optimac` recovery data are left untouched. The Rust
binary cannot restore old trash operations yet; use a previous release for that.
See [migration status](docs/RUST-MIGRATION.md) for remaining work.

## Homebrew development build

The formula is HEAD-only until a Rust release archive and checksum are published:

```sh
brew tap agungdp150/optimac https://github.com/agungdp150/optimac
brew install --HEAD agungdp150/optimac/optimac
```

HEAD builds the remote main branch, so it will include this migration only after
these changes are merged. Local development uses `make build`.

## Development

```sh
make test
make security
make release-check
```
