# Cleanup adapter implementation contract

Status: proposed; no mutation adapter has been certified by this documentation work. This contract resolves the implementation questions in SOME-FEEDBACK.md and is normative alongside CLEANUP-TECH-SPEC.md. It does not claim that untested safety mechanisms exist.

## Registry versus user configuration

[Schema v2 example](rules.example.toml) is the complete user-overlay appendix. Provider policy_version, exact tested tool/app/OS versions, executable provenance, source URLs, verification date, regeneration explanation, activity protocol, permitted operations and reconciliation implementation belong to a compiled adapter registry, never user TOML. Unknown providers, unknown fields, duplicate IDs and unsupported schema versions are rejected. Release packaging must ship the spec, this contract and the example together.

Each registry entry has an explicit certification state. Initially it is inventory-only (or guidance-only). Empty tested-version sets authorize no mutation. Before activation, record minimum and maximum tested versions plus an explicit tested-version allowlist, parser fixtures and macOS/architecture matrix. Being between minimum and maximum is insufficient unless explicitly certified. Above/below range, unparsable output, unknown version or changed executable means inventory/guidance only. No user override unlocks mutation. Probe failure never triggers installation. Native tools use their supported version command; applications use bundle metadata and verified signing identity. Malformed metadata is unknown. CI must test every certified version; upstream changes require a registry update.

## Executable trust

For read-only Phase 1 inventory probes, the lightweight policy in CLEANUP-TECH-SPEC.md applies instead of the enrollment requirements below. The full requirements below remain mandatory before Step 3 enables new deletion operations. Inventory probes do not certify tools for mutation.

Never search arbitrary PATH or execute a candidate merely to find its version. Initial location candidates are Apple tools under /usr/bin, /usr/sbin and /bin; Homebrew under /opt/homebrew/bin or /usr/local/bin; Docker CLI inside /Applications/Docker.app/Contents/Resources/bin. Other tools and version-manager installs require an explicitly selected installation root and trust enrollment before any subprocess runs. Location alone is not authenticity.

Inspect every ancestor and symlink hop using no-follow metadata, limit resolution to 16 hops, and pin final device/inode, owner, mode and content digest. Accept root/current-user ownership; reject group/other write access, unexpected ACL writers, project/cache/download roots and unrecognized symlink destinations. Homebrew symlinks may resolve only into the enrolled Homebrew prefix/Cellar. Apple and signed application executables require valid designated signatures checked with Security.framework and a release-pinned identity; no invented Team IDs. Homebrew/user-owned binaries require explicit installation trust, not a claim that codesign verifies all packages.

Scripts are supported only by an adapter-specific interpreter/launcher recipe with pinned trusted interpreter and dependent runtime; an arbitrary shebang, env-based interpreter, shim or wrapper is rejected. Disable executable configuration/plugins where supported; otherwise discovery stays metadata-only. Use allowlisted environment variables, a private neutral cwd and no network bootstrap. Hash/stat/signature changes invalidate plans and enrollment. This protects against accidental PATH substitution, not a malicious process already controlling the user's account; that limitation must be documented.

## Activity and mutation certification

Activity is active, idle or unknown. Process-name absence and an empty lsof result are only negative observations, never proof of idle. Record exact executable identity, PID/start time, target scope and probe time. Unknown blocks direct mutation. The registry must certify either a lock held through mutation that cooperating writers also honor, or a native operation whose atomic/concurrent semantics protect the selected scope. User assertions that an app is closed do not replace this evidence.

| Provider family | Concrete observations and gate |
| --- | --- |
| Xcode DerivedData | Inspect target-related Xcode, xcodebuild, SourceKitService, sourcekit-lsp, swift-frontend, clang, ibtool and actool processes and open descriptors. No shared writer lock has been established here: absence remains unknown; direct removal is deferred until a positive exclusion protocol is certified. |
| Homebrew | Inspect enrolled brew processes and native lock behavior for selected cleanup scope. Do not invent a global Cellar lock or acquire unknown lock files. Mutation certification requires demonstrating native cleanup's lock/concurrency contract on tested releases; otherwise inventory/guidance. |
| npm, pip, Go, Poetry, CocoaPods, Composer, Yarn, pnpm, Bun, Deno, Dart | Inspect matching tool/runtime processes and descriptors under resolved roots. No common safe lock is assumed. Whole-cache mutation stays disabled until each native operation's concurrency behavior is documented and tested; no-process is unknown. |
| uv | Resolve `uv cache dir` and UV_CACHE_DIR, inventory exact scope; future cleanup uses native `uv cache clean`, never manual cache surgery. Certify installed-version locking behavior before enabling it. See [uv cache safety](https://docs.astral.sh/uv/concepts/cache/). |
| Gradle / JetBrains | Running Gradle daemons or relevant IDE/build processes block direct cleanup. Lock-file presence/absence alone is not proof. Guidance-only until a supported protocol is certified. |
| App caches / shared containers | Query running applications and executable identities for all attributed owners; inspect open descriptors. No universal exclusive app-cache lock is known. Generic providers remain inventory-only; app-specific mutation requires its own certification. |
| Docker images / containers | Query exact IDs and current references/state, recheck daemon identity, then native removal without force. Daemon refusal is a safe blocked outcome. Running/paused/restarting containers block removal. BuildKit cache remains inventory-only because builder activity/scope is not certified. |
| Trash / recovery | Pin exact reviewed entries by parent descriptor, device/inode, type, size and nanosecond mtime/ctime; lock OptiMac recovery operations. Never re-enumerate to expand the approved set. Stage exact selected entries, revalidate, and reject changed descendants. Open-file uncertainty blocks deletion. No Finder-wide empty operation. |
| Models, Maven, Conda, browser downloads, Cargo, SwiftPM, project artifacts | Inventory/guidance until per-provider activity and dependency proofs are certified. No inherited generic “idle” claim. |

This deliberately makes first-delivery coverage broader than mutation coverage. A release must not advertise a permanent-capable example rule as a currently executable adapter.

## Native postconditions and interruption reconciliation

Before native mutation, save complete baseline inventory, pinned roots/IDs, tool/daemon identity, scope and command digest. Partial discovery cannot authorize a whole-tool purge. After every exit, including success, timeout and signal, repeat bounded read-only discovery with the same identities. Missing entries are observed absent, not assumed deleted by OptiMac. New/changed entries invalidate whole-scope completion; never retry to remove them.

| Adapter operation | Postcondition / reconciliation observation |
| --- | --- |
| brew cleanup | Repeat same dry-run and selected formula-version/cache inventory. Reviewed candidates must be absent and retained installs present. Changed dry-run scope is partial/unknown, not a reason to rerun cleanup. |
| npm cache clean | Rescan resolved cache scope and version-defined cache metadata. Do not require unrelated npm state to disappear. |
| pip cache purge | Repeat `pip cache info` and wheel/HTTP inventory under the same interpreter/cache root. |
| go clean -cache / -modcache | Re-resolve GOCACHE/GOMODCACHE without changing configuration; rescan the selected cache separately. Preserve unrelated Go locations. |
| poetry cache clear | Repeat named-cache listing and content inventory; verify virtualenv subtrees unchanged. |
| pod cache clean | Repeat pod cache list and selected pod/version inventory. |
| composer clear-cache | Re-resolve selected cache paths under controlled config and rescan each; preserve vendor/auth state. |
| yarn cache clean | Repeat version-specific local/global cache inventory and confirm the reviewed scope only. |
| deno clean | Repeat DENO_DIR/cache inventory and supported native cache report. |
| pnpm store prune | Repeat store inventory and supported status reporting; only certified unreferenced targets qualify. |
| dart pub cache clean | Rescan selected package-cache scope and protected activated-tool state. |
| uv cache clean | Repeat uv cache dir and supported cache inventory using the same installed tool. |
| docker image rm / container rm | Query exact IDs with inspect/list against the pinned daemon. Explicit not-found means absent; stopped/present means not removed; daemon failure means unknown. |
| Ollama rm (future) | Use `ollama rm <reviewed model>` only, then list/inspect that name and referenced blobs through the same local server. Never directly delete blobs. |
| Hugging Face (future) | Use a version-certified noninteractive cache-management API/CLI with reviewed repo/revision IDs; no nested interactive delete-cache prompt, no manual snapshot symlink traversal. Until that versioned interface is certified, inventory-only. |

All rows require a version-specific parser and test before execution. Exit zero plus verified postcondition permits completed; known mixed outcomes become partial; inability to inspect becomes needs_reconciliation. Nonzero exit remains recorded even if targets are absent. Store observations, not a fictitious rollback. Protected-state changes are a safety incident that stops the batch.

Cancellation before spawn is cancelled_before_mutation. After spawn, record cancellation_requested, stop scheduling, send SIGTERM to the isolated process group, wait two seconds, then SIGKILL if required. Record cancelled_during_mutation and reconcile even if the command claims no work occurred. Timeout follows the same path. No automatic replay after restart or ambiguous completion.

## Docker Desktop locality

Resolve requested context using `docker context show`/`inspect` with environment overrides inspected first. Conflicting DOCKER_HOST/DOCKER_CONTEXT values require explicit context selection; never silently choose. Execute subsequent calls with explicit pinned context and remove overriding endpoint variables. See [Docker contexts](https://docs.docker.com/engine/manage-resources/contexts/).

For initial mutation support require all of: a trusted signed Docker Desktop installation and enrolled CLI; context endpoint resolving to the tested current-user Desktop socket (`~/.docker/run/docker.sock`, with `/var/run/docker.sock` accepted only as the expected link); verified socket ownership/ancestor permissions; socket-serving process attributable to that signed Desktop installation; native info reporting the certified Docker Desktop OS identity; stable daemon ID and socket identity immediately before each mutation. Context name and server-reported OS alone are insufficient. Unsupported layouts, inability to attribute the socket, proxies and any inconsistency block mutation. Local malicious impersonation under the same user is outside the threat model; this is not cryptographic remote attestation. Colima, Podman, Rancher Desktop and remote contexts are inventory-only.

Add docker-tagged-images inventory, class user_state, risk review. Explicit selection can create a future removal plan, but execution remains blocked until its own reference/provenance checks are certified. No implicit removal of tags or force option.

## Attribution, overlap and project evidence

Generic app caches are inventory-only in the first release. A curated registry maps bundle IDs to verified app identities/cache layouts. Local bundle metadata and Launch Services lookup may suggest matches, but cannot certify cache contents. User attribution can annotate display only. Resolution routes a subtree to an already certified specific adapter; it never elevates the generic provider. Files directly under Library/Caches are reported as unclassified files, not selected. Generic Application Support scanning is excluded; only app-specific verified subdirectories are eligible.

Resolve overlaps before planning: protected/excluded subtrees first, then certified specific adapters, project discovery, and generic discovery. Multiple specific providers claiming a subtree require a registry-declared owner relationship; otherwise block the intersection for both. Remove claimed subtrees from parent totals/plans and display their owner. Parent units that cannot be safely split are blocked. Deduplicate physical identities, and do not sum shared allocation as exclusive bytes. Test Poetry/virtualenv, project/Cargo, shared browser and generic/specific conflicts.

Project mutation requires a recognized output contract plus complete Git index and working-tree inventory. Use trusted git, disabled fsmonitor/hooks/config includes, NUL-delimited `git ls-files --cached --stage -z` and `git ls-files --others -z` in explicit repository context, without executing project code. Preserve every indexed path (including ignored-but-tracked), submodule, unmerged entry and unknown untracked descendant. A shallow clone still has an index; missing/corrupt index, unsupported VCS or missing git blocks mutation. Ignore patterns do not prove disposability. Modified vendored dependencies require a complete verified package-integrity baseline; absent baseline blocks removal of the unit. `.venv`, node_modules and generic build directories therefore remain inventory-only until that evidence exists. [Git ls-files](https://git-scm.com/docs/git-ls-files).

Maven remote-origin metadata is a hint, not proof of current re-download availability or absence of local edits: inventory-only. Conda extracted packages remain guidance-only until all registered/configured environments and cache links can be enumerated and identity-checked; unknown external environments block mutation. A dry run alone is insufficient. Download archives are a separate review unit.

## UI, capacity, access and recovery

Keep the existing Ratatui/Crossterm TUI; no GUI or web frontend. A frame is a completed terminal render following an input event. Measure p95 from input receipt to render completion under fixture scans. Measure cancellation from cancellation-flag publication to last newly scheduled mutation, excluding completion of already-running OS calls; also record keypress latency separately.

Phase 1 uses statfs only. Phase 2 may add Foundation capacity values through an in-process macOS-only `objc2-foundation` bridge, autorelease pool and typed optional/error results; no Swift helper or subprocess. The bridge requires dependency/API review, privacy-reason review, mocked failure tests and real-volume comparisons before activation. Until then secondary estimates display unavailable; purgeable storage stays unquantified.

FDA guidance is a heuristic: enumeration EPERM/EACCES within an enabled known privacy-sensitive root, with no evident POSIX owner/mode/ACL explanation, permits wording “macOS privacy restrictions may prevent access.” Failed ACL inspection means unknown, not proof. Other denials suggest ownership/ACL/sandbox investigation. Never probe protected Mail/Messages just to diagnose FDA; never claim FDA status from one successful read.

Recovery lists individual payloads, provider/plan IDs, dates, logical/allocated totals and journal health. Support purge by ID, plan, provider or all through a fresh reviewed snapshot of exact IDs. `--all` selects existing records, never future arrivals. Every purge requires named-item/selection-count and irreversible-consequence confirmation; no unattended flag. At 10 GB show a persistent nonblocking warning; never auto-expire. `auto_expire_days` must be zero in v2. A failed space/journal preflight blocks new moves. Same-volume rename does not duplicate payload bytes but can require metadata space.

No mutation without synced intent and staging metadata. If intent/staging creation fails, leave source untouched. If rename outcome is uncertain or completion sync fails, stop the batch, preserve source/staging evidence and report needs_reconciliation; if the journal itself cannot record that state, startup reconciliation derives it from durable intent. No automatic retry or permanent-delete fallback. Revalidate each target at staging time; identity mismatch skips that target and continues. Global trust/journal failures stop the batch. Expired plans stop scheduling new targets and require fresh review.

## Schema details

All bytes are decimal (MB = 1,000,000; GB = 1,000,000,000). Risk conditional_cache permits grouping recommendations for certified reproducible caches; review requires individual unit selection and consequence display; protected forbids mutation. Both first two still default to no selection and require confirmation. Unknown content is protected when the built-in policy cannot permit removal (system/private state); otherwise review inventory pending assessment. `high_risk` forces named-object plus consequence confirmation and cannot be lowered below provider minimum. User_state permanent operations and holding-area purges require it.

Closed overlay enums: operation = inventory/recovery/permanent/guidance; age_basis = mtime_hint/not_applicable; size_basis = aggregate_review_unit; on_denied = report_partial. Other values fail validation. `not_applicable` requires explicit min_age_days = 0. Guidance always uses it and thresholds do not filter instructions. For logs, size applies to the entire log directory/rotation group; individual entries still require age/closed-file checks. file_removal_default selects recovery when the user begins direct-file removal; it never overrides an explicit confirmed plan or invents an unsupported backend.

Basename patterns use a small specified matcher: full-component, case-sensitive matching, only `*` (zero or more characters) and `?` (one character), dot literal, no slash, `**`, braces, brackets or escapes. Reject unsupported syntax. Thus com.apple.* matches a prefix, not org.foo.com.apple.bar. Unicode matching is by scalar value; non-UTF8 basenames are inventory-only and cannot bypass exclusions.

Quick and Deep share eligible scope; Deep extends enumeration budgets, not deletion policy. UI shows all discovered entries in pages of 50, sorted by known allocated size then stable path; unknown-size entries remain reachable. Explicitly report undiscovered/partial roots. Deep thresholds are configurable, bounded by checked integer ranges. Hashing defaults to 10 GB read or 60 seconds per scan, whichever first; report bytes read and remaining groups every 100 ms. Re-stat before/after hashing and final comparison with device/inode/size/nanosecond mtime/ctime; changes discard the match, never silently rehash into an old plan. Budget exhaustion yields incomplete groups and no removal recommendation.

## Delivery gates and risk register

Phase 0 bounds the reported 20 GB/2 GB discrepancy using an explicitly authorized read-only inventory/report comparison; no cleanup is authorized by this document. If the user's machine/report is unavailable, record the gap as unverified and use reproducible fixtures, with no parity claim. Phase 1 benchmarks Quick/Deep budgets on representative Intel/Apple Silicon fixtures and publishes omissions/performance before Phase 3. Mutation certification is a separate release gate per adapter, not implied by completing the inventory phase.

`optimac clean` remains a supported read-only alias to `clean scan` through the same implementation. No deprecation/removal is planned; JSON remains opt-in and text gains explicit completeness labels.

| Risk | Mitigation / release gate |
| --- | --- |
| False cache attribution | Generic inventory-only; curated specific adapter and exclusions |
| Concurrent build/download | Positive exclusion or certified native concurrency; otherwise blocked |
| Wrong host granted FDA | Display launch context, retry per-root probes, never claim global access |
| Native version drift | Exact certified versions, pinned executable, parser fixtures |
| Recovery/journal exhaustion | 10 GB warning, explicit purge, durable intent, stop on sync failure |
| Recovery mistaken for savings | Separate moved bytes from observed volume delta in every result |
| Incomplete scan broadens purge | Complete baseline required for native whole-tool operations |

First-release non-reclamation gaps are explicit: generic app caches, simulator runtimes, backups/archives, system logs, Gradle, Maven, Conda extracted packages, Minecraft, Time Machine snapshots, generic Application Support and uncertified native adapters. Tagged images and installed Cargo tools are inventoried only. Preserving these gaps is preferable to claiming unverified mutation support.
