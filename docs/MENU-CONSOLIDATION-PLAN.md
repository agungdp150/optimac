# OptiMac menu consolidation plan

Status: approved design direction; implementation pending.

## Objective

Replace the current 17-entry home menu (16 actions plus Quit) with six main
choices: **Clean, Uninstall, Analyze, Duplicates, Status, and More**. Detailed tools live inside
those choices. Quit becomes a keyboard command in the footer.

Keep the cat theme and responsive Rust UI. The reference screenshot provides a
useful model for a compact home screen: a small header, a short list of actions,
one-line descriptions, and keyboard hints. OptiMac's labels must describe what
its own implementation actually does.

This supersedes the earlier proposal to reduce the menu to 13 entries. Six home
choices reduce the initial decision load while retaining useful tools in submenus.

## Home screen

| Choice     | Short description                 | Destination                                            |
| ---------- | --------------------------------- | ------------------------------------------------------ |
| Clean      | Review files and move to recovery | Scope, grouped file review, confirmation               |
| Uninstall  | Review and remove an app bundle   | Eligible app list, bundle review, confirmation         |
| Analyze    | Explore files and disk usage      | Directory overview, large files, project artifacts     |
| Duplicates | Find matching file contents       | Scope selection and bounded exact-content comparison   |
| Status     | Check system health               | Overview, memory and swap, storage, security checks    |
| More       | Recovery and maintenance tools    | Restore recovered items, updates, login-item inventory |

Do not add an Optimize choice while the available behavior is diagnostic only.
Do not describe Clean as freeing disk space: moving files into recovery retains
their data on disk. Permanent deletion and recovery-space reclamation are outside
this menu change.

Example home layout:

```text
 /\_/\  zZ    OptiMac
( o.o )       mac care
 > ^ <

 > 1. Clean       Review files and move to recovery
   2. Uninstall   Review and remove an app bundle
   3. Analyze     Explore files and disk usage
   4. Duplicates  Find matching file contents
   5. Status      Check system health
   6. More        Recovery and maintenance tools

 ↑/↓ move · Enter select · q quit
```

Use a compact header at smaller terminal sizes. Hide secondary descriptions before
sacrificing the selected row or keyboard hints. Avoid a large empty detail panel
on the home screen; reserve detail panels for results and file/app review.

## Workflows and submenus

### Clean

Combine Deep Clean Preview and Run cleanup into one workflow:

1. Show the selected scope and allow the user to change it. Start with the scope
   passed to `optimac ui <path>`, or the current directory when none was supplied.
2. Explain which locations will be scanned: personal files within the selected
   scope plus the explicitly supported home cache roots. Changing the personal
   scope must not conceal the additional cache locations.
3. Scan in the background, showing progress, cancellation and incomplete results.
4. Display nonempty categories with counts and logical sizes: Documents,
   Programming, Browser, AI tools, Images, Videos, Audio, Archives, Installers and Other.
5. Open a category to review individual files. Eligible old cache candidates are
   preselected; personal files are never preselected. Provide clear-all and
   select-caches actions.
   Categories describe file types; they do not imply files are safe to discard.
6. Review the exact selected paths and total size. Only an explicit confirmation
   authorizes recoverable moves. Cancellation returns to selection.
7. Show completed moves, skipped items, failures and recovery IDs. Offer a direct
   route to Restore recovered items.

Keep a visible Restore recovered items shortcut on the Clean landing screen too.
Preview is always part of Clean, never a separate home action. CLI `clean` remains
a preview unless a separately designed execution contract is introduced.

Preserve existing cleanup rules: age restrictions for supported caches, restricted
personal-file roots, sensitive-path exclusions, active-file checks, identity
validation, recovery records, and no automatic permanent deletion. Browser review
continues to cover supported old cache files only, excluding profiles, history,
cookies and credentials. Scope selection must not expand mutation permissions.

### Uninstall

Keep the existing eligible app selection, bundle review and explicit confirmation.
Explain that only the selected app bundle moves; personal data and shared support
files remain. Do not use “remove completely.”

Provide Restore recovered items from the Uninstall landing screen and operation
results. Return from restore to the invoking screen, rather than always jumping
to the home menu. Keep ownership, active-file, path and recovery checks unchanged.

### Analyze

| Submenu            | Behavior                                                   |
| ------------------ | ---------------------------------------------------------- |
| Directory overview | Bounded metadata summary for the selected scope            |
| Large files        | Largest scanned regular files, with readable logical sizes |
| Project artifacts  | Build/dependency candidates with observed size totals      |

Show the scope before starting each scan. Preserve scan limits and skipped/error
counts. A limit reached means partial results, not a completed full-directory scan.
These tools remain read-only; cleanup remains a separate reviewed workflow.

### Duplicates

Keep Duplicates directly accessible from the home menu. Show the selected scope
before starting a bounded exact-content comparison. Present matching paths and
sizes, along with skipped entries, errors and comparison limits. Filename similarity
alone must never count as a duplicate.

Preserve the current read-only behavior and `duplicates` CLI command. No automatic
deletion is introduced by this menu change. Test direct navigation from Home,
cancellation, partial results and returning to the same home selection.

### Status

Replace the fragmented System Status, Hidden Space, Memory diagnostics and Doctor
home entries with one Status area:

| Section         | Information                                                                                     |
| --------------- | ----------------------------------------------------------------------------------------------- |
| Overview        | macOS version, CPU count, physical memory and disk summary                                      |
| Memory and swap | `vm_stat` and `sysctl vm.swapusage`                                                             |
| Storage         | Filesystem capacity and local Time Machine snapshot listing                                     |
| Security checks | FileVault, SIP, Gatekeeper and firewall status                                                  |
| AI tools        | Installed integrations, cache footprint, locally observed usage and supported quota information |

Load details on demand rather than launching every external command when the home
screen opens. Share results where practical so visiting multiple sections does not
repeat the same expensive checks. Show when results were collected and provide a
refresh action. Keep cancellation, per-command timeouts and bounded output.

Read failures should appear as unavailable checks, never as healthy results.
Snapshot listings are inventory, not a calculation of reclaimable space. Memory
statistics must not imply that high usage alone proves memory pressure. No purge,
service restart or process termination is added.

### AI tools: cleanup, usage and quota

Add AI-tool support within Clean and Status, keeping the six-choice home menu.
Initial integration candidates are Codex, Claude Code, Grok tooling and Antigravity.
These are targets for investigation, not a claim that every product offers a CLI,
cache-cleaning contract, usage API or quota endpoint. Identify the exact installed
product, publisher and version before enabling a provider adapter; a similar binary
name is not enough. Support additional tools through the same adapter interface.

| Location                    | Workflow                                                                                                        |
| --------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Clean → AI tools → provider | Review verified regenerable cache files, ages, counts and sizes; select exact files; confirm recoverable moves  |
| Status → AI tools           | List detected products and supported capabilities; open a provider detail view                                  |
| Provider detail → Storage   | Separate disposable cache size from protected session/configuration data                                        |
| Provider detail → Usage     | Show supported local token observations, period and coverage                                                    |
| Provider detail → Quota     | Show provider-reported remaining limits and reset times only when an authorized, supported source supplies them |

#### Cache policy

Discover only documented or locally verified locations using bounded metadata
inspection. Do not classify an entire tool home directory as a cache. Maintain
versioned allowlists for proven regenerable files and explicit exclusions for
credentials, login state, sessions, conversation history, memories, checkpoints,
project settings, rules, skills, plugins, MCP configuration and user-created assets.
A directory named cache is not sufficient evidence that its contents are disposable.

Nothing starts selected. Explain what will be rebuilt or downloaded after a move.
Skip live files, uncertain file types and unsupported versions. If an operation
needs a provider-native maintenance command, review that command's behavior before
adding it; never silently substitute an irreversible command for a recoverable move.
Do not delete transcripts or usage records as a side effect of cache cleanup.

#### Usage and quota semantics

Present these as separate concepts:

- **Local token usage:** observations from a documented local counter or supported
  telemetry format. Show the date range, source, last update and whether records are
  incomplete. Deduplicate events and respect the provider's token-field definitions.
- **Provider/account usage:** reported account totals for a named product, account
  scope and period. API billing usage and subscription allowance must not be merged
  unless the provider explicitly defines that relationship.
- **Remaining quota:** authoritative limits and remaining amounts from a supported
  source. Preserve the unit: tokens, requests, credits, messages or percentages.
  Do not subtract locally observed tokens from an assumed plan allowance.
- **Reset time:** show the provider's reset timestamp and timezone when available;
  never infer it from a guessed subscription schedule.
- **Estimated cost:** optional, only with a verified model/rate basis and clearly
  labeled as an estimate. Omit it when model, price, cache pricing or billing scope
  is unknown. It is not a remaining quota calculation.

Use explicit states such as Available, Local estimate, Partial, Stale, Not connected,
Unsupported and Unavailable. Missing data is never displayed as zero usage or
unlimited quota. Cache cleanup does not reset server-side quotas or recover tokens;
state this in the cleanup flow when quota information is shown nearby.

#### Access and privacy

Local discovery must work without network access and must not launch interactive
agent sessions, submit prompts or consume inference tokens. Do not invoke untrusted
executables solely because they appear on PATH. Prefer documented metadata and
verified vendor commands whose side effects are understood.

Remote account/quota retrieval is an explicit per-provider connection/refresh flow.
Use supported authentication and the minimum required permissions. Do not scrape
private endpoints or extract credentials from another tool's storage. Never print
secrets or copy them into operation journals. Do not upload prompts, transcripts,
source code or filesystem paths for usage reporting. If reading local usage records
is necessary, explain the source, read only needed fields and avoid exposing content.

Use on-demand refresh, timeouts, cancellation and cached results with timestamps.
Respect rate limits and backoff; no background polling from the home screen.

#### Adapter contract and rollout

Each adapter declares independent capabilities: discovery, cache review, local
usage, account usage and quota. Unsupported capabilities remain visible as such;
one missing API must not prevent storage inspection. Define typed normalized
results carrying source, product/version, account scope, units, period, timestamp
and confidence/coverage. Keep provider-specific parsing outside navigation code.

1. Verify official documentation and local installation layouts for each candidate.
   Record supported versions, verified cache roots, exclusions and data sources in
   a capability matrix before enabling cleanup.
2. Implement discovery and read-only storage inventory, with fixture tests for
   missing installations, alternate locations, permissions and unfamiliar versions.
3. Enable cleanup per adapter only after exclusion, active-file and restore tests
   pass. Other providers can remain inventory-only.
4. Add local usage where the format is supported; test duplicate events, partial
   files, schema changes, model changes and timezone boundaries.
5. Add account/quota integrations only where supported authorization exists. Test
   authentication failure, expired access, rate limits, stale values and unavailable
   fields. Never invent a quota display to make provider cards look complete.

This extends the earlier navigation-only scope. Do not broaden cleanup permissions
until each provider's rules are verified, or delay the six-item menu release while
waiting for unsupported quota integrations. Deliver adapters incrementally and
report exactly which capabilities work for each provider.

### More

| Submenu                 | Behavior                                                     |
| ----------------------- | ------------------------------------------------------------ |
| Restore recovered items | Existing Rust recovery selection and non-overwriting restore |
| Updates                 | Homebrew outdated inventory; no automatic installation       |
| Login items             | Launch-agent/daemon inventory; no enable/disable controls    |

Remove Threat Scan and Leftover data review from the menu. Filename signature
matches are not a security assessment, and listing Application Support folders does
not establish that their contents are orphaned. Do not retain empty placeholders.

## Navigation and responsive behavior

- Arrows and j/k navigate lists; Enter opens the selected destination.
- Esc goes back one level. Returning preserves the previous selection and scope.
- On file review, Space toggles an individual file and r opens the exact-file
  confirmation. Keep these existing shortcuts visible on the relevant screen.
- During work, c requests cancellation. Leaving a work screen cancels its job;
  partial results and completed moves remain accounted for.
- q and Ctrl-C request exit from any screen. Mutation jobs must stop scheduling
  new moves and finish recording any move already in flight before normal exit.
- Confirmation is a distinct screen/state. Navigation or resize must never
  confirm an operation, broaden selection, or run a different action.
- Resize recalculates layout and scroll bounds without resetting selection,
  launching work, or changing the reviewed plan.
- The home screen should fit at 80×24. At 40×10 use the compact header and shorter
  hints; at 20×5 keep the selected item visible with scrolling. Extremely small
  terminals must remain usable for back/quit without panicking or overflowing.

## Implementation design

### Replace index-based dispatch first

The current UI and CLI couple behavior to positions in `ITEMS`, `COMMANDS` and
numeric match arms. Do not implement this change by repeatedly renumbering them.

Introduce typed actions and shared menu definitions. For example:

```rust
enum Action {
    Clean,
    Uninstall,
    DirectoryOverview,
    LargeFiles,
    Duplicates,
    ProjectArtifacts,
    SystemOverview,
    MemoryStatus,
    StorageStatus,
    SecurityChecks,
    AiToolsStatus,
    Restore,
    Updates,
    LoginItems,
}
```

Give each menu entry a label, description and typed destination. A destination may
open a submenu or launch an action. Keep the highlighted row index local to a
list; it must never serve as the action identifier. The example above is a design
sketch, not a requirement to use those exact variant names.

Separate navigation state from job state and reviewed mutation plans. Use typed
screens or a navigation stack to return to the correct parent. Associate worker
results with the job that produced them so a stale result cannot overwrite a
newer screen. Render functions must not launch jobs or mutate files.

### CLI compatibility

Menu consolidation does not require renumbering or removing useful CLI commands.
Map CLI names directly to typed actions and preserve existing argument validation:

| CLI                                             | Planned handling                                                        |
| ----------------------------------------------- | ----------------------------------------------------------------------- |
| `inspect`, `analyze`, `duplicates`, `artifacts` | Preserve existing read-only behavior                                    |
| `clean`, `apps`                                 | Preserve preview/inventory contracts; menu provides confirmed workflows |
| `status`, `doctor`                              | Overview and security checks respectively                               |
| `space`, `optimize`                             | Retain as read-only aliases for storage and memory diagnostics          |
| `updates`, `login`, `log`, `history`            | Preserve existing supported behavior                                    |
| `threats`, `orphans`                            | Retire with a clear reason and exit code 2; do not say “not yet ported” |
| Unsupported mutation flags                      | Continue to reject explicitly                                           |

Do not add destructive CLI capabilities as part of this navigation change.
Legacy recovery data remains untouched; consolidation does not implement legacy
restore import.

## Delivery sequence

1. Add typed actions and shared destination definitions. Port current dispatch to
   them before rearranging the menu, with tests for every action mapping.
2. Build the six-choice home screen and Analyze, Status and More submenus. Preserve
   back navigation, selection and compact rendering.
3. Combine cleanup preview and execution into the single Clean workflow. Add scope
   selection and direct recovery links from Clean and Uninstall.
4. Consolidate status collection, on-demand sections and refresh behavior. Remove
   the weak threat/orphan menu tools and implement explicit CLI retirement messages.
5. Update README, CLI help, screenshots, roadmap and changelog to match the final
   behavior. If released as 0.2.1, update Cargo.toml, Cargo.lock and VERSION together;
   do not bump the version merely for this planning document.
6. Implement AI-tool discovery and capability reporting, then deliver verified cache
   and usage/quota adapters incrementally as described above.
7. Run unit, integration and terminal tests before rebuilding or installing.

Likely implementation files: [UI module](../src/ui/mod.rs), [actions module](../src/actions/mod.rs),
[main.rs](../src/main.rs), and [CLI tests](../tests/cli.rs). New navigation modules
may be appropriate. Existing recovery rules remain the baseline. AI-tool cleanup is a separately
validated extension, with provider-specific exclusions and the same confirmation
and recovery guarantees.

## Acceptance criteria

| Check                | Required result                                                                                     |
| -------------------- | --------------------------------------------------------------------------------------------------- |
| Home menu            | Exactly six choices; cat theme retained; Quit only in footer                                        |
| Duplicates access    | Available directly from Home; retains exact-content comparison and read-only CLI behavior           |
| Feature mapping      | Every retained tool is reachable through its intended typed destination                             |
| Back navigation      | Esc restores the correct parent, selection and scope                                                |
| Clean                | Preview, individual selection, exact-path review and explicit confirmation in one workflow          |
| Recovery access      | Reachable from More, Clean and Uninstall, with correct return behavior                              |
| Read-only navigation | Opening menus, changing scope, resizing and cancelling never mutate files                           |
| Status               | Sections load on demand; errors and timestamps are visible; refresh does not overlap jobs           |
| AI-tool safety       | Credentials, sessions and configuration survive cache cleanup; unverified files are excluded        |
| AI-tool usage        | Every value identifies its source, period, unit and freshness; unsupported quota is explicit        |
| AI-tool privacy      | No prompts, transcripts or source code uploaded; remote refresh requires an explicit connection     |
| CLI                  | Supported commands retain their contracts; retired tools explain retirement with exit code 2        |
| Resize               | Home, submenus, lists, results and confirmation tested at 120×40, 80×24, 40×10, 20×5 and tiny sizes |
| Background work      | Cancellation, rapid navigation and stale results cannot launch or confirm the wrong action          |
| Mutation lifecycle   | Back/quit stops new moves and retains recoverable records for completed/in-flight work              |
| Validation           | Formatting, tests, Clippy, security checks, release checks and PTY smoke tests pass                 |

Use disposable fixtures for cleanup/uninstall/restore tests. Exercise the full
flow through nested navigation, confirmation cancellation, successful recovery,
restore collision, resize and quit. Do not test destructive behavior on user data.

## Future features

Reintroduce orphan analysis only with app identity evidence, coverage of relevant
application locations, meaningful size calculations and explicit uncertainty.
Even then, a missing installed app does not make its documents disposable.

A future threat feature needs a defined detection method, maintained evidence and
clear limits. A filename match alone is not sufficient.

Targeted storage probes may be added inside Analyze or Status when they report
concrete sizes and scope honestly. Keep the six-choice home structure as new
features are added, rather than expanding it again.
