# Review: Cleanup coverage and disk reclamation (v0.2.0, macOS Rust)

## Overall assessment

This is a strong, safety-first specification. It correctly identifies that the current 2 GB result is an accounting and semantics problem first, and a coverage problem second. The separation of inventory from mutation, the refusal to equate logical bytes with reclaimed bytes, the insistence on revalidation, and the explicit acknowledgment of unknown state are all right.

The document is not yet approvable. Several core safety properties are asserted but not specified: how "idle" is detected per provider, how native-command partial completion is reconciled, how executable paths are validated, and what the version-compatibility policy is. The referenced `rules.example.toml` is absent, so the schema contract cannot be reviewed. The generic application-cache provider is described as having "recovery as the proposed operation" while also stating that unknown owners block mutation, which leaves the provider effectively inventory-only without a resolution workflow.

Below is a structured review.

---

## Blocking issues

These should be resolved before the spec is considered implementable.

### 1. The example schema is missing

The document repeatedly references `rules.example.toml` as "a proposed configuration contract" but never includes it. The "Extended rule catalog" is a prose table, and schema v2 is described in normative prose. Without the actual TOML, the following cannot be verified:

- Whether `class` and `risk` enums are closed.
- Whether `operation` is per-rule or per-provider.
- How `[scope]` and per-rule `scope` interact syntactically.
- Whether `[rule.docker]` is a special case or a general sub-table pattern.
- How `exclude_name_patterns` and `exclude_paths` are declared.

**Recommendation:** Include `rules.example.toml` (schema v2) as an appendix, with every provider ID represented and every field populated or explicitly omitted. The spec says "All example rules remain `enabled = false`" — show that.

### 2. Idle/activity detection is underspecified per provider

The spec states:

> Activity detection is three-state: active, idle, unknown. Unknown blocks direct mutation. Process-name matching and lsof alone cannot guarantee absence of races; adapters need tool locks where available and conservative refusal otherwise.

But it does not say, for any provider, what is checked. This is the single most important safety mechanism for direct mutation. Without it, "provider idle" is an unimplementable claim.

Examples that need concrete definitions:

| Provider          | Required check(s)                                                                                                                                                                                                   |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Xcode DerivedData | Which processes? `Xcode`, `xcodebuild`, `SourceKitService`, `swift-frontend`, `ibtool`, `actool`? Any lock file?                                                                                                    |
| Homebrew          | `brew` process? Any formula lock? Cellar lock?                                                                                                                                                                      |
| Gradle            | Daemon presence? `.gradle` lock files? `GRADLE_USER_HOME` locks?                                                                                                                                                    |
| Docker            | `docker ps` for running containers? BuildKit activity? Daemon identity check?                                                                                                                                       |
| npm/pip/Go        | No reliable lock — is the policy "unknown unless no process named X"? That is weak.                                                                                                                                 |
| Trash             | No process; but new entries can arrive. How is the reviewed set pinned? (The spec says "Never empty newly added entries outside the reviewed plan" — how is that enforced? By inode + mtime snapshot at plan time?) |

**Recommendation:** Add a per-provider "activity evidence" column to the provider tables, listing the exact process names, lock files, and native probe commands used, plus the fallback behavior when evidence is unavailable. State explicitly that "no matching process" is _unknown_, not _idle_, unless the adapter has a positive idle signal.

### 3. Version-compatibility policy is undefined

Each provider definition is said to contain "supported tool versions," and the delivery gates require a "supported-version matrix." But the policy for an _unknown_ version is not stated. Homebrew, npm, pip, Go, Docker, and most apps update frequently. Without a policy, adapters either break silently or block all mutation after every upstream release.

**Recommendation:** Define, per adapter:

- Minimum tested version.
- Maximum tested version.
- Behavior for a version above the maximum: inventory-only? Block mutation? Warn and require explicit override?
- Behavior for a version below the minimum: block entirely?
- How version is detected (command output, file path, `--version` parsing) and how failure to parse is handled.

State the default for all providers: **unknown version ⇒ inventory only, mutation blocked.**

### 4. Native-command reconciliation is unspecified

The spec says:

> A killed native command yields unknown/partial completion until reconciled.

and

> Never replay a destructive native command automatically.

But it does not define reconciliation for any native operation. For each native command offered (`brew cleanup`, `npm cache clean --force`, `pip cache purge`, `go clean -cache`, `go clean -modcache`, `poetry cache clear`, `pod cache clean`, `composer clear-cache`, `yarn cache clean`, `deno clean`, `docker image rm`, `docker container rm`), the spec must state:

- What post-condition is checked after a normal exit.
- What post-condition is checked after timeout or signal.
- Whether the check is a re-run of the read-only discovery, a native status command, or a filesystem observation.
- What state is recorded when reconciliation cannot determine outcome.

**Recommendation:** Add a "reconciliation" column to the native-command adapters, or a dedicated subsection per provider. Do not ship a mutation adapter without a defined reconciliation path.

### 5. Docker locality verification is hand-wavy

> A Unix socket alone does not prove locality; verify supported Docker Desktop identity. Ambiguous/proxied or remote endpoints block mutation.

What exactly is verified? Candidates:

- `docker context inspect` for a context named `desktop-linux` or similar.
- `docker info --format '{{.OperatingSystem}}'` containing "Docker Desktop".
- Socket path under `~/.docker/run/docker.sock` or `/var/run/docker.sock` with a known symlink target.
- Daemon ID stability across probe and execution.

**Recommendation:** Specify the exact checks, the exact pass/fail conditions, and the behavior when checks are inconclusive (block mutation). Also state explicitly that Colima, Podman, Rancher Desktop, and remote contexts are out of scope for mutation in the first release.

### 6. Executable-path validation is undefined

> Use direct argument arrays, validated executable paths, controlled environment and no shell interpolation.

"Validated" is not defined. A malicious `brew`, `npm`, `docker`, or `go` earlier in `PATH` would otherwise be executed. The spec's own test list includes "malicious executable/config resolution," so the mechanism must exist.

**Recommendation:** Define, per executable:

- Known trusted locations (e.g., `/opt/homebrew/bin/brew`, `/usr/local/bin/brew`).
- Whether custom installs are permitted, and if so, how they are validated (ownership, permissions, codesign, explicit user approval).
- Whether `PATH` resolution is ever used, and if so, how it is constrained (e.g., refuse if the resolved path is in a user-writable directory before a system directory).
- Behavior when the executable is a symlink or wrapper script.

Default should be: **resolve from a known-location allowlist; refuse otherwise; allow explicit user-supplied path only after ownership/permission checks.**

### 7. UI technology is unspecified

The spec targets "UI input-to-frame p95 below 100 ms" and describes "Clean opens a provider summary," "Detail view," "Final review," etc. It does not say whether the UI is a TUI (e.g., `ratatui`), a native GUI, or a web UI. This affects:

- Dependency selection.
- Test harness design.
- Whether the performance target is meaningful.
- Whether Foundation API access (see #9) is in-process or via a helper.

**Recommendation:** State the UI technology in the Scope or Architecture section. If it is a TUI, say so and define "frame" accordingly. If a GUI is planned, name the framework or mark it as undecided with a decision gate.

### 8. Generic app-cache attribution has no resolution workflow

The spec says:

> This is class `unknown_content`, risk `review`, with recovery as the proposed operation and no automatic selection. Unknown owner/activity blocks mutation until resolved.

and

> A directory name resembling a bundle ID is attribution evidence only; unrecognized names remain visible as unknown owners.

If unknown owner blocks mutation, and no resolution workflow is defined, the generic provider can never mutate anything. "Until resolved" implies a resolution mechanism, but none is described.

**Recommendation:** Define the attribution-resolution workflow:

- Is there a curated bundle-ID → application registry shipped with the release?
- Can the user manually attribute a directory to an application?
- Is attribution resolved via Launch Services (`lsregister`), `mdfind`, or a local database?
- What is the trust level of each attribution source?
- Does resolution unlock recovery only, or permanent deletion as well?

Without this, either the provider is inventory-only (state that explicitly), or the spec is incomplete.

### 9. Foundation API access from Rust is unaddressed

The spec requires:

> Also query Foundation URL resource values on that volume: `volumeAvailableCapacityKey`, `volumeAvailableCapacityForImportantUsageKey`, and `volumeAvailableCapacityForOpportunisticUsageKey`.

These are Objective-C Foundation APIs. The project is a Rust CLI. The spec defers "required-reason API/privacy-manifest obligations for the eventual packaged implementation" but does not say which phase adds these keys, or how they are accessed (e.g., `objc2-foundation`, a small Swift/ObjC helper, or a subprocess). It also does not say whether Phase 1 accounting includes them or only `statfs`.

**Recommendation:** State the implementation approach and the phase. If Phase 1 is `statfs`-only, say so and label the Foundation keys as a later-phase enhancement. If Phase 1 includes them, specify the FFI mechanism and the test approach.

### 10. Recovery growth and purge-all are unspecified

The spec correctly adds `recovery purge <id>`. But:

- Is there a `recovery purge --all`? Per-plan? Per-provider?
- Is there a warning or UI state when recovery exceeds a size threshold?
- Can recovery be inspected and selectively purged item-by-item, or only per-plan?
- What happens when recovery fills the volume? The staging and journal requirements imply handling, but recovery itself is not bounded.

The current tool's core problem is that it "never permanently deletes" and has "no empty-recovery operation." The spec fixes the operation but not the lifecycle.

**Recommendation:** Specify recovery management: listing, per-item and bulk purge, size warnings, and behavior when the recovery volume is full.

### 11. `class` vs `risk` distinction is unclear for `conditional_cache`

The mapping table permits `rebuildable_cache` with risk `conditional_cache` or `review`. The prose says:

> Regeneration known; review required when provenance/cost/sharing is uncertain.

But both values still require review before mutation (`auto_select = false`). What is the operational difference between `conditional_cache` and `review`? Is it:

- UI emphasis only?
- A difference in which confirmation is required?
- A difference in whether an adapter may propose a suggestion?
- A difference in whether the item appears in Quick vs Deep scan?

Similarly, `unknown_content` may be `review` or `protected`. When is unknown content `protected`?

**Recommendation:** Define each risk value operationally: what it permits, what it requires, and how it affects UI and selection. If the difference is cosmetic, consider collapsing to a single value.

### 12. Overlap conflict resolution between specific providers is undefined

> Specific validated providers take precedence over generic inventory for the same subtree.

This handles specific vs generic. It does not handle specific vs specific. Examples:

- A pip virtualenv inside a project root also matched by project-artifact discovery.
- A Cargo target directory inside a project root also matched by the Cargo provider.
- A Docker build cache under a project root.
- A Poetry cache that shares a hierarchy with virtualenvs (the spec explicitly warns about this).

**Recommendation:** Define a precedence order (e.g., by provider ID priority list) and a rule for partial overlaps (specific provider wins over the claimed subtree; generic provider displays the remainder and the owner). Add tests for specific-vs-specific overlap.

### 13. "Tracked files" detection is undefined

> Preserve tracked files, modified vendored dependencies and unreproducible environments.

How is "tracked" determined?

- `git ls-files`? What if the project is not a git repo?
- What if git is not installed?
- What about files tracked by other VCS (Mercurial, SVN)?
- What about files that are `.gitignore`d but still source (e.g., generated files that are committed by some teams)?
- What about untracked new source files that are not yet committed?

The same question applies to "modified vendored dependencies" — how is modification detected?

**Recommendation:** Specify the detection mechanism, the fallback when it is unavailable (probably: block mutation for that project), and the test fixtures.

### 14. FDA applicability determination is vague

> Do not treat EPERM/EACCES as proof of missing Full Disk Access... For applicable denials, guide the user to System Settings > Privacy & Security > Full Disk Access.

How is "applicable" determined? There is no public API to query FDA status. The spec needs a heuristic.

**Recommendation:** Specify heuristics such as:

- Path is under a known TCC-protected location (`~/Library/Mail`, `~/Library/Messages`, `~/Library/Safari`, `~/Library/Application Support/com.apple.TCC`, etc.).
- Error is `EPERM`/`EACCES` on metadata enumeration, not on file content.
- No ACL or ownership explanation is found.
- Otherwise, suggest ACL/ownership/sandbox investigation instead.

State that the heuristic is not proof and that the UI wording reflects uncertainty.

### 15. Tagged Docker images are not inventoried

The Docker section inventories "dangling images, stopped containers and build cache as separate categories" and says:

> Never include volumes, networks, running containers, tagged unused images or `docker system prune` implicitly.

Tagged unused images are excluded from _implicit_ prune, which is correct, but they are not listed as an inventoried category. Many users have tens of GB of tagged old images. If the goal is coverage, they should be inventoried (as user state or review) and selectable explicitly.

**Recommendation:** Add a "tagged images" inventory category with explicit selection and no default selection. State that they are user state unless the adapter can prove provenance (e.g., pulled from a registry with a matching digest).

### 16. `uv` is mentioned but missing from the provider catalog

> Keep existing Cargo and uv coverage visible...

`uv` is not in the extended catalog. It has a cache directory (`UV_CACHE_DIR`, default platform-specific) and a native `uv cache clean`. It should either be added as a provider or the mention should be removed.

**Recommendation:** Add a `uv-cache` provider row with discovery (`uv cache dir`), scope, and native cleanup command, following the same review-only initial stance.

### 17. Size units are inconsistent

The document uses:

- "100 MB aggregate size" (eligibility policy).
- "100,000,000 bytes everywhere" (normative schema).
- "10 MB" (Downloads).
- "500 MB" (Large old files).
- "20 GB" / "2 GB" (problem statement, decimal presumably).

**Recommendation:** Pick one convention (decimal MB = 10^6 bytes is suggested by "100,000,000 bytes") and apply it everywhere. State the convention once in the schema section.

---

## Significant issues

### 18. Plan validity and revalidation timing

> Proposed plan validity: ten minutes maximum; every target must still be revalidated immediately before mutation.

For a plan with thousands of targets, "immediately before" per target is unclear. Is revalidation:

- Per target, interleaved with mutation?
- Per batch, before any mutation?
- Per provider?

If per target, mutation of target N may occur minutes after the plan was created, even if target N was validated at plan time. The spec should state whether revalidation is per target at staging time (which is the safe reading) and what happens when a target fails revalidation mid-batch (skip that target, continue; never broaden).

**Recommendation:** Clarify: "Each target is revalidated at the moment it is staged; failure skips that target and records the reason; the rest of the plan continues; the plan is not re-expanded."

### 19. Staging disk-full behavior

Tests include "disk-full journals," but the behavior is not specified. What happens when:

- The journal cannot be written before mutation?
- The staging directory fills during a move?
- The journal fills after mutation but before completion is recorded?

**Recommendation:** State the invariant: no mutation occurs unless the journal and staging intent are durably recorded. If staging fails partway, the unit is left in staging and marked `needs_reconciliation`; the plan stops; no automatic retry.

### 20. Cancellation semantics for native commands

> A killed native command yields unknown/partial completion until reconciled.

What does "killed" mean here? SIGTERM then SIGKILL after a grace period? What is the grace period? Does cancellation during a native command count as partial completion even if the command had not started mutating? The spec should distinguish "cancelled before mutation" from "cancelled during mutation" — the former may be safely retryable, the latter requires reconciliation.

**Recommendation:** Define cancellation states and the corresponding journal entries.

### 21. Glob semantics for `exclude_name_patterns`

> documented glob matching

Which glob? `*` vs `**`? Case-sensitive? Does `*` match `/`? Is it `globset`, `fnmatch`, or shell-style? The spec must name the semantics.

**Recommendation:** Specify a library and semantics, or state that only `*` and `?` on basenames are supported, case-sensitive, no `**`.

### 22. Long-tail inventory presentation

> Inventory the long tail by descending allocated-size estimate, with logical size and incomplete counts also visible.

How many? All children? Top N? Paginated? What is N? Does the user see every immediate child of `~/Library/Caches`, or only the largest?

**Recommendation:** Define a default page size and whether the UI supports "show all." If the provider is inventory-only (see #8), "show all" is cheap and should be the default for reviewability.

### 23. Application Support caches are not covered

The generic provider covers `~/Library/Caches` and sandbox/group containers. Many applications store caches under `~/Library/Application Support/<app>/Cache` or similar. The spec excludes Application Support implicitly by not covering it, but does not state that this is a deliberate coverage gap.

**Recommendation:** Add an explicit note: "Application Support is out of scope for generic cache inventory because it contains user state; app-specific adapters may cover known cache subdirectories." Otherwise, the 20 GB mystery may partly be there.

### 24. Simulator runtime deletion is deferred without a coverage note

> Manage runtimes through Xcode until supported adapter behavior is tested.

Simulator runtimes are often multiple GB. Deferring them means OptiMac will not reclaim them. This is acceptable, but it should be stated as a coverage gap, not buried.

**Recommendation:** Add a "Coverage gaps in first release" list: simulator runtimes, iOS backups (guidance only), system logs, Gradle direct pruning, Minecraft, Time Machine snapshots, Application Support caches, tagged Docker images (if #15 not adopted), Maven local artifacts.

### 25. Duplicate hashing budget

> Hashing reads contents, so use a separate cancellable I/O budget.

What is the budget? How is it measured? Is it bytes read, time, or both? Does the user see progress?

**Recommendation:** Specify the budget (e.g., "default 10 GB or 60 s per duplicate group, whichever is lower; cancellable; progress reported"). Also specify how "stable identity" is verified after hashing (re-stat inode, size, mtime; re-hash on change).

### 26. Maven recoverability heuristic

> Preserve anything not proven remotely recoverable.

How is remote recoverability proven? Presence of `_remote.repositories`? A matching POM with a repository URL? A checksum match against a configured repository? Maven's local repository mixes downloaded and locally installed artifacts, and the spec acknowledges this. It should state the heuristic.

**Recommendation:** Default to inventory-only unless `_remote.repositories` (or equivalent) proves remote origin for every file in the artifact. Never mutate an artifact with a locally installed POM.

### 27. Conda linked-environment detection

> Conda explicitly warns package cleanup may break environments linked by symlinks.

How does the adapter detect linked environments? By scanning `envs/*/conda-meta` for hardlinks/symlinks? By invoking `conda list --revisions`? The native `conda clean` has a dry-run, but the adapter must interpret it.

**Recommendation:** Specify the detection method and default to guidance-only until it is implemented and tested. Do not offer `conda clean --packages` without linkage evidence.

### 28. Scan budgets may be inadequate for the stated coverage

The proposed Quick budget is 10,000 entries or two seconds per provider, 30 seconds globally. The current implementation's 15 seconds across all roots is called out as a limitation. The new budget is 30 seconds globally, with 10,000 entries per provider. For a provider like `app-user-caches` with hundreds of children and nested caches, 10,000 entries may be exhausted before the long tail is sized. For Deep, 250,000 per provider and 1,000,000 globally may be adequate, but the spec does not justify the numbers or describe how they were chosen.

**Recommendation:** State that these are starting points to be benchmarked, as the spec does, and add a delivery gate: "Phase 1 benchmarks Quick/Deep budgets on representative machines and publishes results; budgets are revised before Phase 3."

### 29. `optimac clean` deprecation path

> Keep existing `optimac clean` read-only.

What is the migration story? Does `optimac clean` remain indefinitely as an alias for `clean scan`? Is it deprecated with a warning? Does it change output format? The spec should state this to avoid two divergent scan paths.

**Recommendation:** State: "`optimac clean` remains read-only and delegates to the Phase 1 inventory; its output is labeled as legacy; it is deprecated in favor of `clean scan` and will be removed in a future major version."

### 30. Risk register and pre-implementation comparison are missing

The document has no risk register. For a destructive tool, the top risks should be explicit:

- False attribution of a directory to a rebuildable cache.
- Race with an active build or download.
- FDA grant applied to the wrong binary.
- Native command behavior drift across versions.
- Recovery volume exhaustion.
- User misunderstanding of "Moved to recovery" as "reclaimed."

The comparison protocol is described but not assigned to a phase. The 20 GB vs 2 GB question should be resolved (or bounded) before Phase 3.

**Recommendation:** Add a risk register with mitigations, and add "Phase 0: run the comparison protocol on the reporting user's machine with consent" as a gate for Phase 3.

---

## Minor issues

- The document would benefit from a one-paragraph TL;DR at the top, since it is long.
- The `wget-state` provider is odd: it appears to respond to a user request for "history" that may have been a misunderstanding. Consider demoting it to a note in the exclusions rather than a provider row.
- `com.apple.*` exclusion should state prefix-only matching (i.e., `com.apple.foo` excluded, `org.foo.com.apple.bar` not excluded).
- The generic `app-user-caches` provider says "immediate child directories" — state what happens to files directly in `~/Library/Caches` (probably ignore, but say so).
- "Quick and Deep scans" should state whether Deep is a superset of Quick or a separate mode.
- The performance target "stopping new work within 250 ms of cancellation" needs a measurement method: is it measured from keypress to no-new-mutations, or from cancellation signal to last worker join?
- The `huggingface-cache` provider should state whether it invokes `huggingface-cli delete-cache` (interactive) or reimplements the cache-management logic; "supported cache management" is ambiguous.
- The `ollama-models` provider should state whether it invokes `ollama rm <model>` or manipulates the store directly; the latter is explicitly forbidden, so the former must be the contract.
- The class/risk table's `holding_area` class is not represented in any provider row in the catalog; either add a row (Trash/recovery are covered elsewhere) or remove the class from the mapping table.
- "Partial scans cannot authorize whole-tree deletion" is correct but should also be stated for native whole-tool purges: a partial inventory of a provider must not authorize a whole-tool command.

---

## Questions for the author

1. Is the UI a TUI, GUI, or both? Which framework?
2. What is the exact Docker Desktop locality check, and what happens on Colima/Podman/Rancher Desktop?
3. How is "tracked" determined for project artifacts when git is absent or the repo is shallow?
4. What is the attribution-resolution workflow for unknown app-cache owners?
5. Which phase adds the Foundation capacity APIs, and via what FFI mechanism?
6. Is there a `recovery purge --all`, and what is the warning behavior when recovery grows large?
7. What is the operational difference between risk `conditional_cache` and `review`?
8. What is the version-compatibility policy for a tool version above the tested maximum?
9. For each native command, what is the reconciliation check after a timeout?
10. Are tagged Docker images in scope for inventory and explicit selection?
11. Is `uv` a provider, and if so, what is its discovery and native cleanup contract?
12. Are the Quick/Deep budgets validated by benchmark before Phase 3, or are they shipped as-is?
13. What is the deprecation timeline for `optimac clean`?
14. Is the 20 GB vs 2 GB comparison a Phase 0 gate, or is it deferred?
15. What is the size threshold for warning the user that recovery is consuming significant space?

---

## Recommendations, prioritized

1. **Add `rules.example.toml` (schema v2) as an appendix.** Without it, the schema is not reviewable.
2. **Add per-provider activity-evidence and reconciliation columns** to the provider tables. This is the core safety contract.
3. **Define the version-compatibility policy** (min/max, unknown-version behavior) and the default (unknown ⇒ inventory only).
4. **Specify executable-path validation** and the default trusted-location policy.
5. **Specify the Docker locality check** concretely, and state the out-of-scope runtimes.
6. **Define the attribution-resolution workflow** for generic app caches, or state that the provider is inventory-only.
7. **State the UI technology** and the phase that adds Foundation capacity APIs.
8. **Add recovery lifecycle management**: listing, purge-all, size warnings, volume-full behavior.
9. **Clarify `conditional_cache` vs `review`** operationally, or collapse the risk values.
10. **Add a coverage-gaps list** for the first release: simulator runtimes, iOS backups, system logs, Gradle direct pruning, Application Support caches, tagged Docker images, Maven local artifacts, Time Machine snapshots.
11. **Add a risk register** with the top safety risks and mitigations.
12. **Add Phase 0**: run the comparison protocol on the reporting user's machine (with consent) to bound the 20 GB vs 2 GB gap before Phase 3.
13. **Resolve the unit inconsistency** (decimal MB vs MiB) and state the convention once.
14. **Add `uv-cache`** as a provider or remove the mention.
15. **Add tagged Docker images** as an inventoried, explicitly selectable category, or state why they are excluded.

---

## What the spec gets right (and should not lose)

- **Truthful accounting**: separating logical, allocated, recovered, moved-to-recovery, and observed-free-space-change is exactly right.
- **Move-first, delete-second**: staging, identity rechecks, and descriptor-relative deletion are the right primitives.
- **No automatic replay of destructive native commands** after restart.
- **Three-state activity** (active / idle / unknown) with unknown blocking mutation.
- **Explicit acknowledgment of unknown age** rather than inferring from mtime.
- **Docker daemon-reported bytes vs host disk change** kept separate.
- **Foundation `volumeAvailableCapacityForImportantUsageKey`** shown separately and not mislabeled as purgeable.
- **Deny-over-allow** for exclusions, and protected roots that user config cannot override.
- **Fixture-based tests**, never the developer's real cache.
- **Phased delivery** with separate reviewable changes.

The spec's instincts are correct. The gaps are in making the safety properties concrete enough to implement and test. Resolving the blocking issues above would move this from "proposed" to "implementable."

TOML FEEDBACK:

# Review: `rules.example.toml` (schema v2)

This resolves the largest blocking gap from the previous review — the schema is now reviewable. The file is internally consistent, every rule is `enabled = false`, and the operation/class/risk combinations mostly match the spec's normative tables. Below is the review of this artifact specifically, followed by the updated status of the blocking issues.

---

## What this file gets right

- **All rules disabled.** Every `[[rule]]` has `enabled = false`, and `[defaults]` reinforces it. No accidental mutation on load.
- **All operations review-only.** `operation` values are `inventory`, `recovery`, `permanent`, or `guidance` — no implicit execution grants.
- **Docker sub-table.** `[rule.docker]` with `context`, `endpoint_policy`, `allow_remote` matches the spec's per-rule Docker contract.
- **Class/risk pairings mostly valid.** Spot-checking against the spec's mapping table:
  - `rebuildable_cache` → `conditional_cache` / `review` ✓
  - `generated_output` → `review` ✓
  - `logs` → `review` ✓
  - `holding_area` → `review` ✓
  - `user_state` → `review` / `protected` ✓
  - `unknown_content` → `review` / `protected` ✓
- **Project artifacts use 90 days / 100,000,000 bytes.** Matches the spec's "everywhere" requirement.
- **Large old files use 180 days / 500,000,000 bytes.** Matches the spec.
- **Crash reports inherit 30 days.** Matches the spec.
- **Downloads use 90 days / 10 MB.** Matches the spec.
- **Generic app caches exclude `com.apple.*`.** Matches the spec's default exclusion.

---

## Blocking issues in this file

### B1. Missing provider metadata fields

The spec says each provider definition contains:

> an ID, policy version, supported tool versions, discovery method, root constraints, exclusions, regeneration explanation, activity/lock checks, operation scope, source URLs and verification date.

The TOML contains only `id`, `provider`, `class`, `risk`, `enabled`, `operation`, `scope`, and exclusions. Missing:

- `policy_version`
- `supported_tool_versions` (min/max)
- `source_urls`
- `verification_date`
- `regeneration_explanation`
- `activity_checks` / `lock_paths`

**If these live in built-in Rust code, say so explicitly in the TOML header and the spec.** If the TOML is meant to be the full "configuration contract," these fields must be present (even if read-only from the user's perspective). The current ambiguity means a reader cannot tell whether the TOML is authoritative or a user-overlay.

**Recommendation:** Add a comment block at the top stating: "Provider metadata (policy version, supported tool versions, source URLs, verification date, activity checks, regeneration explanation) is defined in the built-in adapter registry and is not user-overridable. This file configures only user-selectable fields." Or add the fields with a `[metadata]` sub-table per rule.

### B2. Missing `[recovery]`, `[scan]`, and `[ui]` sections

The spec requires:

- **Recovery lifecycle** — "Add selected purge with durable completion state." No `[recovery]` section exists. Required fields: `warn_threshold_bytes`, `purge_requires_confirmation`, `auto_expire_days` (default 0 / disabled).
- **Scan budgets** — "Proposed Deep budget: 250,000 entries per provider, 1,000,000 globally, two minutes globally, explicitly extensible by the user." No `[scan]` section exists. Without it, "explicitly extensible by the user" is impossible.
- **Long-tail inventory page size** — "Inventory the long tail by descending allocated-size estimate." No `[ui]` section. Required: `page_size`, `show_all_by_default` for inventory-only providers.

**Recommendation:** Add:

```toml
[recovery]
warn_threshold_bytes = 10000000000
purge_requires_confirmation = true
auto_expire_days = 0

[scan]
quick_max_entries_per_provider = 10000
quick_max_seconds_per_provider = 2
quick_max_seconds_global = 30
deep_max_entries_per_provider = 250000
deep_max_entries_global = 1000000
deep_max_seconds_global = 120

[ui]
inventory_page_size = 50
show_all_inventory_default = true
```

### B3. Glob, age, and size basis semantics are undeclared

The file uses:

- `age_basis = "mtime_hint"` and `age_basis = "not_applicable"`
- `size_basis = "aggregate_review_unit"`
- `on_denied = "report_partial"`
- `exclude_name_patterns = ["com.apple.*", "group.com.apple.*"]`

None of these enums are documented. A reader cannot tell whether:

- `mtime_hint` means "use mtime but treat it as unreliable" (likely) or something else.
- Other valid `age_basis` values exist (`atime_hint`, `btime`, `ctime`, `trash_time`).
- `aggregate_review_unit` is the only `size_basis` value, or whether `per_file` / `logical_only` are valid.
- `on_denied` accepts other values (`block`, `continue`, `abort`).
- `com.apple.*` is glob or regex. In glob, `.` is literal and `*` does not cross `/`. In regex, `.` matches any character. This ambiguity is dangerous for exclusion patterns.

**Recommendation:** Add a header comment enumerating valid values for each enum field, and state the glob library and semantics explicitly. Example:

```toml
# age_basis: "mtime_hint" | "atime_hint" | "creation_time" | "not_applicable"
# size_basis: "aggregate_review_unit" | "per_file" | "logical_only"
# on_denied: "report_partial" | "block" | "continue"
# exclude_name_patterns uses globset semantics: `*` matches any sequence
# except `/`, `?` matches one non-`/` character, `.` is literal.
# Patterns apply to basename components only.
```

### B4. `age_basis = "not_applicable"` still inherits `min_age_days = 30`

Every rule without an explicit `min_age_days` inherits `30` from `[defaults]`. For rules with `age_basis = "not_applicable"` (e.g., `local-trash`, `duplicates`, `quicklook-cache`, `personal-databases`), the inherited value is meaningless but still present. This is a validation hazard: an implementer might apply the threshold anyway.

**Recommendation:** Either:

- Require `min_age_days = 0` explicitly when `age_basis = "not_applicable"`, or
- State in the schema that `min_age_days` is ignored when `age_basis = "not_applicable"`, and add a validation test.

### B5. `docker-stopped-containers` is `user_state` + `permanent` without an explicit high-risk flag

The spec says:

> Stopped containers are user state because their writable layers can contain unique work.

and

> high-risk state destruction requires selecting the named object and confirming its consequence.

The TOML inherits `require_confirmation = true` from `[defaults]`, which is necessary but not sufficient to distinguish this rule from, say, `crash-reports` (also `require_confirmation = true` but much lower consequence). There is no per-rule severity or `high_risk = true` flag.

**Recommendation:** Add `high_risk = true` (or `confirmation = "named_object_plus_consequence"`) to `docker-stopped-containers`, `local-trash`, `optimac-recovery`, and any other `user_state` + `permanent` combination. Define the flag's effect in the spec: it forces the named-object confirmation dialog described in the interface section.

### B6. `cargo` coverage is incomplete relative to the spec

The spec says:

> Cargo | Resolve CARGO_HOME; distinguish downloaded registry archives from source trees, git checkouts, credentials and installed tools.

The TOML has only `cargo-registry`. Missing:

- `cargo-git` (git checkouts in `$CARGO_HOME/git`)
- `cargo-installed` (installed binaries in `$CARGO_HOME/bin`)

Since all three are `inventory`-only (per "Do not expand their mutation behavior in the first delivery"), they are safe to add now and would make the inventory honest.

**Recommendation:** Add `cargo-git` and `cargo-installed` rules with `operation = "inventory"`, `class = "rebuildable_cache"` (git checkouts) and `class = "user_state"` (installed binaries), both `risk = "review"`.

### B7. `uv` is present in the TOML but absent from the spec's provider tables

The spec mentions `uv` only in passing:

> Keep existing Cargo and uv coverage visible, but treat their current fixed roots as legacy rules pending configuration-aware adapter validation.

The TOML has a `uv` rule, but the spec's "Developer package managers" and "Additional developer and model providers" tables do not document `uv`'s discovery (`uv cache dir`), scope, or native cleanup (`uv cache clean`). This is an inconsistency: the TOML implies a contract the spec does not define.

**Recommendation:** Add a `uv-cache` row to the developer-provider table with the discovery command, scope, and native cleanup command, matching the same review-only stance as `cargo-registry`.

### B8. `docker-tagged-images` is absent

The spec's Docker section excludes tagged unused images from _implicit_ prune but does not inventory them. The TOML has no `docker-tagged-images` rule. The previous review flagged this as a coverage gap; the TOML confirms it.

**Recommendation:** Add `docker-tagged-images` with `operation = "inventory"`, `class = "user_state"`, `risk = "review"`, and the same `[rule.docker]` block. This makes the gap visible in the inventory rather than silently omitted.

### B9. No explicit protected rules for Photos, Messages, Keychain, Mail database, credentials, SIP-managed locations

The spec says:

> Protect Keychains, Photos libraries, Mail, Messages, Notes, app databases, credentials, SIP-managed locations and signed app internals.

The TOML has `personal-databases` and `app-bundle-resources`, but no explicit rules for Photos, Messages, Keychain, or Mail database. Protection is enforced at the filesystem level, so this is not a safety hole — but it means the provider summary will not show these as "protected" categories, which is less honest than it could be.

**Recommendation:** Add `photos-library`, `messages-database`, `keychain`, `mail-database`, `sip-managed-locations` rules, all `risk = "protected"`, `operation = "inventory"`, `class = "user_state"` or `"unknown_content"`, `enabled = false`. These cost nothing and make the protected surface visible.

---

## Significant issues in this file

### S1. `download-installers` class `rebuildable_cache` is debatable

The spec says:

> Individually reviewed .dmg/.pkg/.zip files older than 90 days and over 10 MB. ZIP may be a personal archive; extension is not proof of an installer. Default to recovery.

If a ZIP may be a personal archive, its class at discovery time is `unknown_content`, not `rebuildable_cache`. Since the operation is `recovery`, the practical impact is small — but the class label affects the UI's "effect" description and the risk summary.

**Recommendation:** Change to `class = "unknown_content"` or add a comment justifying `rebuildable_cache` (e.g., "the operation is recovery, so misclassification is recoverable; the class reflects the common case").

### S2. Guidance-only rules inherit `age_basis = "mtime_hint"`

Rules like `gradle-cache`, `maven-repository`, `conda-packages`, `bundler-cache`, `wget-state`, `adobe-media-cache`, `dropbox-cache`, `android-cache`, `steam-download-cache`, `teams-cache`, and the Xcode/simulator guidance rules inherit `age_basis = "mtime_hint"`. For guidance-only rules, mtime is irrelevant. This is harmless but inconsistent with the `age_basis = "not_applicable"` pattern used for other guidance rules (`quicklook-cache`, `mail-attachments`, `time-machine-snapshots`).

**Recommendation:** Set `age_basis = "not_applicable"` on all `operation = "guidance"` rules for consistency, or state that the field is ignored for guidance operations.

### S3. `min_size_bytes` inherited by `crash-reports` and `app-logs`

`crash-reports` and `app-logs` inherit `min_size_bytes = 10000000` (10 MB). Individual crash reports and log files are usually much smaller. The 10 MB threshold will hide most of them unless the adapter aggregates by directory. The spec says "Apply size thresholds to each review unit or aggregate cache group" — for logs, the review unit is likely a directory or rotation group, so 10 MB may be appropriate. But this should be stated.

**Recommendation:** Add a comment on `crash-reports` and `app-logs` stating that the review unit is the aggregate log directory, or override `min_size_bytes = 0` if per-file review is intended.

### S4. Redundant exclusion patterns

- `app-user-caches` excludes `group.com.apple.*`, but `~/Library/Caches` children are not named `group.com.apple.*` (those are in `~/Library/Group Containers`). Redundant.
- `sandbox-app-caches` excludes `group.com.apple.*`, but container names are not `group.com.apple.*`. Redundant.
- `group-container-caches` excludes both `com.apple.*` and `group.com.apple.*`. The first is redundant here (group containers are named `group.*`).

These are harmless but suggest the patterns were copied without per-provider reasoning.

**Recommendation:** Tighten the patterns per provider:

- `app-user-caches`: `["com.apple.*"]`
- `sandbox-app-caches`: `["com.apple.*"]`
- `group-container-caches`: `["group.com.apple.*"]`

### S5. `swiftpm-cache` risk `conditional_cache` vs the spec's "verification pending"

The spec says:

> Swift Package Manager | Inventory configured global caches and project build output separately. No blanket `.swiftpm` removal; adapter/source verification remains pending.

"Adapter/source verification remains pending" suggests `risk = "review"`, not `conditional_cache`. Since the operation is `inventory`, the difference is cosmetic, but it affects the UI's effect description.

**Recommendation:** Change to `risk = "review"` to match the spec's language.

### S6. Missing `scope` validation example

The spec says:

> Nonempty per-rule roots must be within the corresponding global user-approved roots; provider overrides must also meet adapter constraints.

The TOML has no example of a nonempty `scan_roots` or an out-of-scope root. The schema validation cannot be verified from the file alone.

**Recommendation:** Add a commented-out example showing a valid nonempty `scan_roots` for `project-artifacts` and an invalid one, with a note that the invalid case must be rejected at load time.

### S7. `[defaults] operation = "inventory"` vs per-rule overrides

Every rule overrides `operation`, so the default is never used in practice. This is fine but makes the default look arbitrary. If a user adds a new rule without `operation`, they get `inventory` — which is safe. Good default, but worth stating in the header: "New rules default to inventory-only; mutation requires an explicit operation."

### S8. `file_removal_default = "recovery"` has no visible consumer

The field is in `[defaults]` but no rule references it. It presumably controls the default for direct file removal in the executor. The spec says "Default direct-file removal remains recoverable through OptiMac recovery." The field is correct but should be documented: "This is the fallback for direct filesystem removal when a rule specifies `operation = "permanent"` but the adapter supports recovery; the user must explicitly choose permanent."

Actually, re-reading the spec, it says "Default direct-file removal remains recoverable through OptiMac recovery. ... Permanent removal needs an explicit operation choice and confirmation." So `file_removal_default = "recovery"` means: when the executor is asked to remove a file directly, it moves to recovery unless the plan explicitly says permanent. This is a safety default and should be documented.

---

## Minor issues

- **Header says "Valid operations: inventory, recovery, permanent, guidance"** — good, but `operation` values in `[defaults]` and per-rule should be validated against this enum.
- **No `schema_version` validation example** — the spec says "strict schema that rejects unknown fields, duplicate IDs and invalid thresholds." A note that `schema_version` must be `2` for this file would help.
- **No duplicate-ID test fixture** — the spec requires duplicate-ID rejection. The TOML has no duplicate IDs, which is correct, but the test should be documented elsewhere.
- **`exclude_paths = []` is repeated in every rule** — verbose but explicit. Fine.
- **`scan_roots = []` is repeated in every rule** — same.
- **`exclude_name_patterns = []` is repeated in every rule** — same. Could be omitted and inherited from defaults, but explicit is safer.
- **`local-trash` `age_basis = "not_applicable"`** — correct, but `min_age_days` inherited as 30 (see B4).
- **`duplicates` `age_basis = "not_applicable"`** — same.
- **`quicklook-cache` `class = "unknown_content"`, `risk = "protected"`** — consistent with the table.
- **`minecraft-data` `class = "user_state"`, `risk = "protected"`** — consistent.
- **`system-logs` `class = "unknown_content"`, `risk = "protected"`** — consistent.
- **`time-machine-snapshots` `class = "unknown_content"`, `risk = "protected"`** — consistent.
- **`app-leftovers` `class = "user_state"`, `risk = "review"`, operation `inventory`** — consistent.
- **`wget-state` `class = "user_state"`, `risk = "review"`, operation `guidance`** — consistent.
- **No `min_age_days` on `local-trash` / `optimac-recovery`** — they inherit 30 but `age_basis = "not_applicable"` means ignored. See B4.
- **No `min_age_days` on `duplicates`** — same.
- **No `min_age_days` on `app-user-caches`** — inherits 30, which is appropriate for `unknown_content` + recovery.

---

## Updated status of previous blocking issues

| Previous blocking issue                         | Status with this file                                                                                                   |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| 1. Missing `rules.example.toml`                 | **Resolved.** File is present.                                                                                          |
| 2. Idle/activity detection underspecified       | **Unresolved.** No activity-check fields in the TOML.                                                                   |
| 3. Version-compatibility policy undefined       | **Unresolved.** No `supported_tool_versions` fields.                                                                    |
| 4. Native-command reconciliation unspecified    | **Unresolved.** Not represented in the schema.                                                                          |
| 5. Docker locality check hand-wavy              | **Partially resolved.** `[rule.docker]` exists, but the exact check is still prose-only.                                |
| 6. Executable-path validation undefined         | **Unresolved.** Not in schema.                                                                                          |
| 7. UI technology unspecified                    | **Unresolved.** No `[ui]` section, no technology statement.                                                             |
| 8. Generic app-cache attribution workflow       | **Partially resolved.** `app-user-caches` exists with exclusions, but the "until resolved" workflow is still undefined. |
| 9. Foundation API access from Rust              | **Unresolved.** No accounting/FFI fields.                                                                               |
| 10. Recovery growth and purge-all               | **Unresolved.** No `[recovery]` section.                                                                                |
| 11. `conditional_cache` vs `review` distinction | **Unresolved.** Both values used; difference still cosmetic.                                                            |
| 12. Overlap conflict between specific providers | **Unresolved.** Not represented in schema.                                                                              |
| 13. "Tracked files" detection undefined         | **Unresolved.** Not in schema.                                                                                          |
| 14. FDA applicability determination vague       | **Unresolved.** No access-preflight fields beyond `on_denied`.                                                          |
| 15. Tagged Docker images not inventoried        | **Unresolved.** No rule.                                                                                                |
| 16. `uv` missing from catalog                   | **Partially resolved.** TOML has `uv`, spec doesn't.                                                                    |
| 17. Size units inconsistent                     | **Partially resolved.** TOML uses bytes consistently; spec prose still mixes MB/GB.                                     |

---

## Recommendations, prioritized for this file

1. **Add a header comment** stating that provider metadata (policy version, supported versions, source URLs, verification date, activity checks, regeneration explanation) lives in the built-in adapter registry and is not user-overridable. This resolves the ambiguity about what the TOML is.
2. **Add `[recovery]`, `[scan]`, and `[ui]` sections** with the defaults proposed above.
3. **Document enum values** for `age_basis`, `size_basis`, `on_denied`, and `operation` in header comments, and state the glob library and semantics for `exclude_name_patterns`.
4. **Add `high_risk = true`** to `docker-stopped-containers`, `local-trash`, `optimac-recovery`, and any other `user_state` + `permanent` rule.
5. **Add `cargo-git` and `cargo-installed`** inventory rules to match the spec's Cargo coverage.
6. **Add `docker-tagged-images`** inventory rule.
7. **Add explicit protected rules** for `photos-library`, `messages-database`, `keychain`, `mail-database`, `sip-managed-locations`.
8. **Tighten exclusion patterns** per provider (see S4).
9. **Resolve `age_basis = "not_applicable"` + inherited `min_age_days`** — either require `min_age_days = 0` explicitly or document that it is ignored.
10. **Change `swiftpm-cache` to `risk = "review"`** to match the spec's "verification pending" language.
11. **Change `download-installers` to `class = "unknown_content"`** or justify `rebuildable_cache`.
12. **Add a `uv-cache` row to the spec's provider tables** so the TOML and spec agree.

---

## Bottom line

The TOML is a credible first draft of the schema and resolves the largest reviewability gap. It is not yet complete enough to be the "configuration contract" the spec describes: the missing metadata fields, recovery/scan/UI sections, enum documentation, and high-risk flagging are blocking for implementation. The class/risk/operation pairings are mostly correct, and the Docker sub-table is a good pattern. With the fixes above, this file would be approvable as the Phase 1 schema alongside the spec revisions identified in the previous review.
