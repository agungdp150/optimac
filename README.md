<div align="center">

<img src="assets/logo.svg" alt="OptiMac cat logo" width="120">

# OptiMac

**A thoughtful terminal companion for taking care of your Mac.**

Inspect your storage, review cleanup candidates, manage app bundles, and keep an
eye on system health—all from a keyboard-friendly Rust interface.

[Getting started](#getting-started) · [Features](#features) · [Safety](#how-cleanup-works) · [Contributing](#contributing)

[![Platform: macOS](https://img.shields.io/badge/platform-macOS-6f9fb8)](https://github.com/agungdp150/optimac)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-ce8b68)](https://github.com/agungdp150/optimac)
[![License: MIT](https://img.shields.io/badge/license-MIT-67c9ba)](LICENSE)

</div>

```text
 /\_/\  zZ    OptiMac
( o.o )       mac care
 > ^ <

 › Clean       Review files and move to recovery
   Uninstall   Review and remove an app bundle
   Analyze     Explore files and disk usage
   Duplicates  Find matching file contents
   Status      Check system health
   More        Recovery and maintenance tools

 ↑/↓ move · Enter select · s scope · q quit
```

<sub>Illustrative menu; the interface adapts to your terminal size.</sub>

> **Current status:** OptiMac is in development. The Rust edition has a different
> feature set from the earlier Go release. Review the [migration notes](#upgrading-from-01x)
> before replacing an existing installation.

## Features

| | What you can do |
| --- | --- |
| **Clean** | Review files grouped by type, select candidates, and confirm recoverable moves. Eligible old caches are preselected; personal files are not. |
| **Uninstall** | Review an eligible user-owned app bundle and move it to recovery. Personal data and shared support files stay in place. |
| **Analyze** | Inspect a directory, find large files, and review project build or dependency artifacts. |
| **Duplicates** | Find files with matching contents using bounded byte-for-byte comparisons. |
| **Status** | View a compact system dashboard plus memory, storage, and security details. |
| **More** | Restore Rust recovery records, check outdated Homebrew packages, and list launch items. |

## Getting started

### Requirements

- macOS and a terminal with an interactive shell.
- Rust **1.93 or newer** to build from source.
- Homebrew only if you want to use the Homebrew installation or update check.

### Build from source

```sh
git clone https://github.com/agungdp150/optimac.git
cd optimac
make build
./bin/optimac
```

To install the built binary to `~/.local/bin/optimac`:

```sh
make install
optimac
```

Make sure `~/.local/bin` comes before any older OptiMac installation in your
`PATH`. If `optimac version` shows an older version, run `rehash` in zsh or start
a new shell. You can also run `~/.local/bin/optimac` directly.

### Homebrew: development build

The formula is **HEAD-only** until a Rust release archive and checksum are
published. This builds the remote `main` branch; local changes must be pushed
before they are available through Homebrew.

```sh
brew tap agungdp150/optimac https://github.com/agungdp150/optimac
brew install --HEAD agungdp150/optimac/optimac
```

## Using OptiMac

Launch the menu in the current directory, or choose a starting scope:

```sh
optimac
optimac ui ~/Downloads
```

| Key | Action |
| --- | --- |
| `↑` / `↓` or `j` / `k` | Move through lists |
| `Enter` / `Esc` | Open / go back |
| `s` | Change scan scope from the home screen or a submenu |
| `Space` | Select or deselect a file in cleanup review |
| `a` / `u` | Select eligible caches / clear the cleanup selection |
| `r` | Review the cleanup selection, or refresh a Status screen |
| `c` | Request cancellation of a running operation |
| `q` | Quit |

The scope applies to personal-file review and analysis. Cleanup also inspects its
explicitly supported home cache roots; changing the scope does not expand which
paths are allowed for cleanup.

### Command-line reports

```sh
optimac inspect ~/Downloads      # Metadata summary
optimac analyze ~/Downloads      # Largest scanned files
optimac duplicates ~/Downloads   # Exact-content comparisons
optimac artifacts .              # Project artifact candidates
optimac clean                    # Read-only storage inventory
optimac clean scan --json        # Structured inventory and coverage status
optimac clean scan --deep        # Larger read-only scan budgets
optimac status                   # Text system report
optimac doctor                   # Security checks
optimac log                      # Rust recovery record inventory
optimac --help
```

CLI reports are read-only. Cleanup execution, app-bundle moves, and Rust recovery
restoration use the interactive menu and require confirmation. Duplicate detection
does not delete files. `threats` and `orphans` are retired because the earlier
checks were insufficient.

## How cleanup works

1. **Scan** the selected scope and supported cache locations. OptiMac groups
   candidates and shows their logical sizes and any scan limits or errors.
2. **Choose** what to move. Eligible cache files older than 30 days are
   preselected. Personal files must be selected individually; a category does
   not mean a file is disposable.
3. **Review and confirm** the exact paths. Press `r` to review, then `y` to
   confirm or `n` to go back.
4. **Restore if needed** using **More → Restore recovered items**. A restore
   will not overwrite an existing destination.

> **Moving files to recovery does not free disk space.** The data remains on the
> same volume so it can be restored. The CLI now offers an explicitly confirmed purge for individual regular-file
> recovery payloads; bulk and directory purge remain unsupported.

Cleanup uses reviewed plans, checks file identity and whether files appear in use,
records recovery information, and refuses a copy-and-delete fallback. It skips
uncertain or inaccessible items. Close apps and developer tools using the selected
files before cleaning; active-file checks cannot prevent an app from opening a file
immediately afterward. OptiMac does not run the interface as root or force a RAM
purge.

Personal-file selection is limited to standard folders—Documents, Downloads,
Desktop, Pictures, Movies, and Music—within the chosen scope. Browser review is
limited to supported old cache locations; browser profiles, history, cookies, and
credentials are excluded. Sensitive folders, symlinks, hard-linked files, app
bundles, photo libraries, and `.env` files are excluded from personal-file cleanup.

## Status and scan limits

The **Status → System overview** dashboard shows CPU load, estimated memory
availability, disk capacity, battery information, and process samples. It refreshes
after completed samples at a three-second interval; press `r` to refresh manually.
Unavailable checks are labeled. Memory availability is an estimate, not macOS
memory pressure. APFS total minus available is not a precise used-space measure.
Live CPU utilization and network throughput are not currently measured.

Scans use bounded work: up to 100,000 entries, 32 directory levels, and a
cooperative 30-second deadline for the general scanner. Other operations have
their own limits. In-flight filesystem calls can take longer to return. Results
may be partial when files are skipped, permissions fail, or a limit is reached.
Network roots, ancestor symlinks, and every cloud provider are not comprehensively
handled; use a local scope and review the reported status.

Sizes are **logical file lengths**, displayed in decimal units such as KB and GB.
They are not allocated or reclaimable space, and hard links may be counted per
path. CLI reports can contain partial results or per-check failures even when the
command itself exits successfully; inspect the report before acting.

## Upgrading from 0.1.x

The Rust edition is a breaking change from the Go version. Existing configuration
and `~/.optimac` recovery data are left untouched, but the Rust version cannot
restore legacy trash operations. Use a previous release if you need to restore
those records. See [migration status](docs/RUST-MIGRATION.md) for details and
remaining work.

## Contributing

Issues, ideas, and pull requests are welcome. Start with the
[contribution guide](CONTRIBUTING.md) and [security policy](SECURITY.md).

```sh
make test           # Format check, tests, and Clippy
make security       # Repository guardrails
make release-check  # Version and packaging checks
```

See the [roadmap](ROADMAP.md) for planned work and the
[changelog](CHANGELOG.md) for release history.

## License

[MIT](LICENSE) © 2026 Luceid

### Storage inventory (first implementation slice)

Clean → Storage inventory and `clean scan` inspect common developer/model paths,
non-Apple children of Library/Caches, and sandbox/group-container cache paths.
Reports distinguish logical bytes, allocated estimates, absent roots, denied access,
and partial scans. Exit status 1 means incomplete inventory; 2 means invalid usage.
No inventory result selects files or authorizes cleanup. Existing Run cleanup
continues to use its separate recovery-only review.

This is default-location discovery, not complete provider support: custom tool
configuration, executable enrollment, native cleanup,
project discovery and adapter certification remain pending. The example rules file is loaded only when passed with `--config`; all of its rules start disabled. Apple generic cache directories are excluded. Allocated sizes may
share APFS extents and do not predict reclaimed space. Scans are metadata-only,
with cooperative budgets; filesystem calls may exceed those budgets.

### Permanently purge an individual recovery file

Run `optimac log` to find a recovery ID, then `optimac recovery purge <id>`
in an interactive terminal. Review the payload and type the exact requested
confirmation. This permanently removes a regular-file payload and retains its
journal. App bundles, directories, hard links and changed identities are refused.
There is no unattended confirmation flag or bulk purge. An interrupted intent
requires inspection; the command does not automatically retry it. Physical space
reclaimed can differ from logical bytes due to snapshots and shared storage.

### Configured read-only scans

```sh
optimac config check docs/rules.example.toml
optimac clean scan --config my-rules.toml --json
```

Minimal configuration:

```toml
schema_version = 2

[scope]
file_roots = ["~/Downloads"]
exclude_paths = ["~/Downloads/keep"]

[[rule]]
id = "large-old-files"
enabled = true
scope = "files"
scan_roots = []
exclude_paths = ["~/Downloads/work"]
exclude_name_patterns = ["*.tmp"]
```

Only enabled rules are scanned when `--config` is provided. Empty `scan_roots`
inherits `file_roots` for file rules or `project_roots` for project rules;
empty effective roots scan nothing. Nonempty rule roots must be within their
approved scope. Provider-scoped rules can narrow their resolved roots, including supported environment overrides, but cannot
redirect them to unrelated directories. Symlink roots/ancestors are refused.
Global and per-rule exclusions are combined and applied before traversal.
Patterns match complete basename components using case-sensitive `*` and `?`.

`project-artifacts` discovers Cargo target, Node node_modules/.next, SwiftPM .build and Gradle build directories only when their parent contains a matching project marker. These are inventory candidates, not certified disposable outputs. `large-old-files` and `duplicates` report metadata totals; age/size eligibility and duplicate hashing remain unavailable through this interface. Unsupported providers report a
blocked status. Configuration operation settings never execute cleanup.
Use `optimac ui --config my-rules.toml` to apply the same rules to Clean → Storage inventory. Configured Quick/Deep entry and time budgets are applied in both interfaces. Other schema settings such as UI pagination and recovery policies are still validated without being applied.

Use `optimac recovery inspect <id>` to inspect an interrupted purge. It reports
payload presence and journal consistency without retrying deletion. A missing
payload without a completion record is reported as requiring reconciliation;
its absence is not assumed to prove a successful purge. This inspection may
create the recovery lock/root if missing, but never changes payloads or intents.

### Provider capabilities

`optimac clean providers` lists the complete proposed provider catalog. Add
`--json` for schema-versioned capability data. Every entry currently has an empty
tested-version list and mutation disabled; inventory/guidance support must not be
interpreted as certification to run the example's proposed cleanup operation.

Inventory reserves entry/time shares for later providers and roots. CLI progress
uses stderr only when it is a terminal; JSON stdout remains machine-readable.
The TUI displays cooperative progress during scans. Reports include access
observations and the full provider capability catalog. A readable root does not
prove that all descendants are accessible. Blocking filesystem calls can exceed
scan and progress deadlines.

Run `cargo run --release --locked --example inventory_benchmark` to benchmark
4,000 disposable fixture files without scanning real caches.
