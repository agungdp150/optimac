# OptiMac

OptiMac 0.2.0 is a Rust-only macOS filesystem inspector and conservative cleanup tool with a responsive
terminal UI. The menu supports reviewed cleanup, app-bundle removal and Rust recovery. No Go toolchain is needed.

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
Menu actions open bounded background jobs; mutations require a reviewed plan and explicit confirmation.
Resize freely; press `c` to cancel scanning and `q` or Ctrl-C to quit.
CLI scans also support Ctrl-C. Run `optimac --help` for usage.

## Scope and limits

Scans inspect metadata on one worker, with a 100000-entry budget, a 32-directory
stack limit and a cooperative 30-second deadline. Observed symlinks, filesystem
boundaries, selected sensitive/cloud directories and macOS dataless entries are
skipped. Filesystem calls already in flight can exceed the deadline. Root paths
are explicit; use local directories. Ancestor symlinks, network roots and all
cloud providers are not comprehensively handled yet.

Sizes automatically use B, KB, MB, GB or larger decimal units (1 KB = 1,000 bytes).
Sizes are calculated from file metadata and remain exact bytes internally. Artifact
directory totals include scanned files only and may be partial.
Sizes are logical lengths, not allocated or reclaimable disk space. Hard links
are counted per path. Output reports errors, skipped entries and budget limits.
Exit codes: 0 command completed (review diagnostic output for partial results),
1 top-level error or cancellation, 2 invalid or unavailable command. `inspect` also
returns 1 for traversal errors or budget limits. Other diagnostic reports may
contain per-check failures or partial results while returning 0. Read-only CLI reports never mutate files; the menu supports confirmed recoverable moves.

## Migration from 0.1.x

This is a breaking Rust-only cutover. The menu and CLI now support:

- `inspect`, `ui` / `menu`, help and version.
- `analyze [directory]`, `duplicates [directory]`, `artifacts [directory]`.
- `clean` (conservative cache preview only), `apps` (inventory only).
- `updates`, `login`, `threats`, `space`, `doctor`, `status`.
- `orphans` (manual support-folder review, not proven orphan detection).
- `optimize` (memory diagnostics, no purge).
- `log` / `history` (Rust recovery-record inventory only).

**Deep Clean Preview** and **Run cleanup** group eligible files as Documents,
Programming, Browser, Images, Videos, Audio, Archives, Installers and Other.
Groups show counts and logical sizes for scanned files, not guaranteed reclaimable
space. Only nonempty groups appear.

Personal files are discovered inside the selected scope within your Documents,
Downloads, Desktop, Pictures, Movies and Music directories. For example:

```sh
optimac ui ~/Downloads
```

Old developer and browser caches are also reviewed from explicit home-cache roots.
Browser profiles, cookies, passwords and history are excluded. Cache candidates
must be older than 30 days; personal files are not considered disposable based on
age or category. Sensitive folders, app bundles, photo libraries, symlinks, hard
links and `.env` files are excluded from personal-file selection.

In **Run cleanup**, arrows highlight a group and Enter opens it. Space selects an
individual file, Esc returns to groups, and `r` reviews the exact selected files.
Nothing is preselected. Confirm with `y`, or use `n` to return. Recovery retains
data on disk, so moves do not reclaim space. Close apps using selected files first;
active-file status is checked again before each move. Scans are bounded and group
totals may be partial; the confirmation includes scan notes.

**Uninstall apps** lists eligible user-owned bundles in `/Applications` and
`~/Applications`. Select one and review before confirming. Only the bundle moves;
personal data and shared leftovers are preserved. Protected or other-owned apps
are excluded. **Restore recovered items** restores a selected Rust recovery ID
without overwriting an existing destination. Existing legacy recovery data is not
imported. CLI execution flags remain disabled; use the reviewed menu workflow.

Recovery records are written before same-volume exclusive moves. Symlink ancestors
are rejected, parent directories opened without following links, and moved identity
is checked. Recovery operations serialize with a filesystem lock. No root elevation,
copy/delete fallback, RAM purge, automatic orphan deletion, or login-item toggling
is performed. Recovery may still need manual attention after external tampering or
hardware failure; review errors and retain the journal.

Other actions remain read-only: updates, launch-item listing, filename threat hints,
manual leftover review, and bounded diagnostics. They are intentionally limited;
threat-name matches are not antivirus verdicts and leftover folders are not proven
orphans. Memory diagnostics replace forced purging. Results support arrow scrolling
and `c` cancellation. A filesystem call already in flight can delay cancellation.

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
