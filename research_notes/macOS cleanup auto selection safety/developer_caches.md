# Developer tool caches on macOS: official cleanup guidance and preselection safety

Scope note: gathered 2026-11-13. Primary sources are official docs, plus local `xcrun simctl help` output from Xcode 27.2 (build 27B52) on the research machine. Single-source or unofficial claims are marked **[single-source]** or **[unofficial]**. Where a doc page does not address something (for example, whether `rm -rf` of the directory is supported), these notes say so instead of guessing.

## Xcode: DerivedData, Archives, DeviceSupport, CoreSimulator, UserData. Which are safe and which hold irreplaceable data?

### Takeaway
DerivedData is a pure rebuildable cache. Apple DTS says it can be deleted at any time with Xcode quit. Old iOS DeviceSupport versions can go if they don't match a device still used for debugging. CoreSimulator should be managed only through Xcode or `simctl`, never by deleting files. Archives hold the only dSYMs that match shipped builds, and Apple says you *must* retain them, so they must never be preselected. No official statement was found on UserData. It holds user customizations and should not be preselected.

### Cited Findings
- **Archives / dSYMs (irreplaceable):** "When archiving your app for distribution, Xcode gathers all binaries and `dSYM` files for your app and stores them inside the Xcode archive." "You must retain the Xcode archive for each build of your app you distribute." "Without this archive, you might not be able to diagnose an issue from crash reports." — [Apple: Building your app to include debugging information](https://developer.apple.com/documentation/xcode/building-your-app-to-include-debugging-information)
- **dSYMs cannot be regenerated:** "A binary and a `dSYM` file are only compatible with each other when they have identical build UUIDs." "If you build two binaries from the same source code but with different Xcode versions or build settings, the build UUIDs for the two binaries won't match." "Keep the `dSYM` files for the specific builds you distribute." — [Apple: Building your app to include debugging information](https://developer.apple.com/documentation/xcode/building-your-app-to-include-debugging-information). These sentences were extracted from Apple's documentation JSON at `developer.apple.com/tutorials/data/documentation/xcode/building-your-app-to-include-debugging-information.json`.
- **DerivedData:** "You can delete `DerivedData` at any time. It grows back as you load and build projects." — Apple Developer Forums accepted answer (Jan 2026), endorsed by Quinn of Apple DTS ("What Scott said plus…") — [Apple Developer Forums thread 812408](https://developer.apple.com/forums/thread/812408). **[single-source; forum, not reference docs]** The forum thread does not say whether "Scott" is Apple staff. Quinn (DTS) explicitly endorses the answer.
- **iOS DeviceSupport:** "In `iOS DeviceSupport` you can delete any OS versions that don't correspond to devices you actually use for debugging." — [Apple Developer Forums thread 812408](https://developer.apple.com/forums/thread/812408) (endorsed by DTS). Third-party guides add that Xcode re-copies the symbols the next time a device on that OS version is connected. **[unofficial]** — [cleanwithcrumb.com](https://cleanwithcrumb.com/blog/clear-xcode-deriveddata-devicesupport-archives/)
- **CoreSimulator:** "For `CoreSimulator`, it's probably best to not monkey with that yourself but instead manage it via either Window > Devices & Simulators in Xcode or the `simctl` command-line tool." — Quinn, Apple DTS, [thread 812408](https://developer.apple.com/forums/thread/812408)
- **DocumentationCache:** "You can delete it, but Xcode will have to rebuild it… If you do delete it, I recommend that you delete `DocumentationIndex` at the same time." — Quinn, Apple DTS, [thread 812408](https://developer.apple.com/forums/thread/812408)
- **Concurrency hazard:** "in all cases, make sure that Xcode is quit before you start deleting stuff behind its back." — Quinn, Apple DTS, [thread 812408](https://developer.apple.com/forums/thread/812408)
- **`xcrun simctl delete unavailable`:** Built-in help text: "Specifying unavailable will delete devices that are not supported by the current Xcode SDK." Usage: `simctl delete <device> … | unavailable | all`. — local `xcrun simctl help delete`, Xcode 27.2. No URL; this is the tool's own help text, and the same command is described in [cleanwithcrumb.com](https://cleanwithcrumb.com/blog/how-to-delete-unavailable-simulators-in-xcode-and-recover-gbs-2026/) **[unofficial]**.
- **Simulator runtimes:** `simctl runtime delete (<identifier>|--notUsedSinceDays <days>|--unusable|--outdated) [--dry-run] [--keep-asset]`. "If runtime is a disk image any booted simulators are shutdown and the disk is unmounted first." `--outdated`: "Delete runtimes that have a newer version available with the same runtime identifier." `--dry-run`: "Print what images would be deleted without actually deleting anything." — local `xcrun simctl help runtime`, Xcode 27.2. No URL.
- **Archives can be exported before deletion:** Third-party guides recommend exporting dSYMs from Window > Organizer before deleting an archive. **[unofficial]** — [cleanwithcrumb.com](https://cleanwithcrumb.com/blog/clear-xcode-deriveddata-devicesupport-archives/)

### Inferences
- **Safe to preselect:** `~/Library/Developer/Xcode/DerivedData` (only when Xcode is not running).
- **Optional, off by default:** DeviceSupport subfolders older than some age or not matching recently connected devices. The app cannot know which devices the user still debugs on, and the cost is a slow re-copy of symbols on the next connect.
- **Run the tool, don't delete files:** For CoreSimulator, run `xcrun simctl delete unavailable` and `xcrun simctl runtime delete --unusable` / `--outdated` (with `--dry-run` first) rather than deleting `~/Library/Developer/CoreSimulator/Devices` paths. `simctl delete all` would destroy simulator app data and user state, so it should never be automatic.
- **Never preselect Archives.** Apple's "must retain" language makes this the clearest irreplaceable item in this survey.
- **Never preselect UserData.** It holds user-authored state such as key bindings, themes, and snippets. This is an inference: no official source was found.
- **Detect the running app:** Process detection for Xcode (and for Simulator.app when touching CoreSimulator) is required before any Xcode-area deletion.

### Gaps
- No Apple reference-documentation page (as opposed to the forum) states DerivedData or DeviceSupport deletion policy. Xcode's own Settings > Locations UI exposes the DerivedData path, but I did not retrieve an Apple help page for it.
- No official statement on `~/Library/Developer/Xcode/UserData` contents or deletability. The forum thread lists it but the answers don't address it.
- No official source confirming the exact on-disk Archives path (`~/Library/Developer/Xcode/Archives`) was retrieved. That path is widely used but not verified here.
- The forum thread has more replies behind "Load more" that I didn't read.

## Homebrew: `brew cleanup`, `--prune=all`, HOMEBREW_CACHE

### Takeaway
`brew cleanup` is the official, supported mechanism. Homebrew already runs it automatically: on every install/upgrade/reinstall for the touched formulae, a full cleanup every 30 days, and download pruning past 120 days. Running `brew cleanup` (optionally `--prune=all`) is low risk because it only removes downloads and *old* versions. The download cache at `~/Library/Caches/Homebrew` is re-downloadable.

### Cited Findings
- `cleanup`: "Remove stale lock files and outdated downloads for all formulae and casks, and remove old versions of installed formulae." It "Removes all downloads more than 120 days old." — [Homebrew Manpage](https://docs.brew.sh/Manpage)
- `--prune`: "Remove all cache files older than specified _days_. If you want to remove everything, use `--prune=all`." `-s/--scrub`: "Scrub the cache, including downloads for even the latest versions." Downloads for installed formulae/casks are still kept. `-n/--dry-run`: "Show what would be removed, but do not actually remove anything." — [Homebrew Manpage](https://docs.brew.sh/Manpage)
- `HOMEBREW_CACHE` default on macOS: `~/Library/Caches/Homebrew`. — [Homebrew Manpage, ENVIRONMENT](https://docs.brew.sh/Manpage). `brew --cache` displays it. — [Homebrew FAQ](https://docs.brew.sh/FAQ)
- `HOMEBREW_CLEANUP_MAX_AGE_DAYS`: "Cleanup all cached files older than this many days." Default `120`. `HOMEBREW_CLEANUP_PERIODIC_FULL_DAYS`: "If set, `brew install`, `brew upgrade` and `brew reinstall` will cleanup all formulae when this number of days has passed." Default `30`. — [Homebrew Manpage](https://docs.brew.sh/Manpage)
- `HOMEBREW_NO_INSTALL_CLEANUP` disables automatic cleanup. `HOMEBREW_NO_CLEANUP_FORMULAE`: "Homebrew will refuse to clean up or autoremove a formula if it appears on this list." — [Homebrew Manpage](https://docs.brew.sh/Manpage)
- "Homebrew automatically uninstalls old versions of each formula that is upgraded with `brew upgrade`", and "periodically performs additional cleanup every 30 days." — [Homebrew FAQ](https://docs.brew.sh/FAQ)

### Inferences
- **Preselection candidate:** `brew cleanup` (or `brew cleanup --prune=all` for the download cache) via the CLI. It honors the user's `HOMEBREW_NO_CLEANUP_FORMULAE` opt-outs and Homebrew's own lock files. Deleting `~/Library/Caches/Homebrew` directly would bypass those and could race a running `brew` process.
- **Don't preselect:** Old *installed* versions removed by `brew cleanup` may be versions a user deliberately kept. The opt-out env var exists for that reason, so the app should run `brew` rather than reimplement it.
- **Run as the user:** Run `brew` as the user, never as root.

### Gaps
- The docs do not say whether hand-deleting `~/Library/Caches/Homebrew` is supported, or describe concurrency with a running `brew` beyond "stale lock files".
- The docs don't say how pinned formulae interact with `cleanup`.

## npm, pnpm, Yarn (classic and Berry), Bun

### Takeaway
npm's cache is "strictly a cache". Clearing it needs `--force` and is "typically unnecessary". It only affects future installs. pnpm documents `pnpm store prune` as harmless. On macOS, the pnpm store and Bun cache feed `node_modules` via hard links or APFS clones, so the supported route is each tool's own prune command. Yarn Berry's per-project `.yarn/cache` may be committed for zero-installs and must not be treated as a disposable cache.

### Cited Findings
- **npm location:** POSIX default `cache` = `~/.npm`. Data lives in `_cacache` inside it. — [npm docs: npm-cache](https://docs.npmjs.com/cli/v10/commands/npm-cache)
- **npm clearing policy:** Clearing "should never be necessary… for any reason other than reclaiming disk space". `clean` "now requires `--force` to run". "npm will not remove data by itself: the cache will grow as new packages are installed." "The npm cache is strictly a cache: it should not be relied upon as a persistent and reliable data store." — [npm docs](https://docs.npmjs.com/cli/v10/commands/npm-cache)
- **npm integrity:** "All data that passes through the cache is fully verified for integrity on both insertion and extraction." `npm cache verify` garbage-collects unneeded data and verifies the index. — [npm docs](https://docs.npmjs.com/cli/v10/commands/npm-cache)
- **npm concurrency:** The npm docs say nothing about concurrent access or deleting `_cacache` by hand.
- **pnpm prune:** "Removes _unreferenced packages_ from the store." "Running `pnpm store prune` is not harmful and has no side effects on your projects." "It is best practice to run `pnpm store prune` occasionally… but not too frequently," since pruned packages may be re-needed, for example after switching branches. — [pnpm: store](https://pnpm.io/cli/store)
- **pnpm prune also removes:** expired `pnx` cache entries ("because they hold hard links into the store"), the package metadata cache, and "the macOS directory-clone cache". With the global virtual store, it garbage-collects `links/`. `pnpm store path` prints the active store directory. — [pnpm: store](https://pnpm.io/cli/store)
- **pnpm hard links:** "pnpm creates hard links from the global store to the project's node_modules folders." "Hard links point to the same place on the disk where the original files are." — [pnpm FAQ](https://pnpm.io/faq)
- **Yarn Berry:** By default Yarn caches everything and shares it across projects (global cache). With `enableGlobalCache: false`, the cache goes to project-local `.yarn/cache`, which "can then be added to Git" to enable zero-installs. — [Yarn: Caching](https://yarnpkg.com/features/caching)
- **`yarn cache clean`:** Removes the local (project) cache by default. `--mirror`: "Remove the global cache files instead of the local cache files". `--all`: "Remove both the global cache files and the local cache files of the current project." — [Yarn CLI: cache clean](https://yarnpkg.com/cli/cache/clean)
- **Bun:** Global cache at "`~/.bun/install/cache`, or the path set by the `BUN_INSTALL_CACHE_DIR` environment variable." On macOS "Bun uses `clonefile`, which is copy-on-write… patching `node_modules/*` in one project can't affect other installations." Bun checks `node_modules` first and only uses the cache or registry for missing packages. — [Bun docs: install cache](https://bun.sh/docs/install/cache)

### Inferences
- **npm:** `~/.npm/_cacache` is a reasonable preselection candidate. The supported route is `npm cache clean --force`. Deleting it should not break installed projects, but this is an inference from "strictly a cache"; the docs don't say so explicitly. Hazard: a concurrent `npm install` writing into `_cacache`. The docs say nothing on this, so the app should avoid it when npm/node processes are running.
- **pnpm:** Prefer `pnpm store prune` over deleting the store. Under POSIX hard-link semantics, deleting the store directory does not delete file data still linked from `node_modules`. However, it discards all of pnpm's dedup, so every project re-downloads on the next install, and pnpm's metadata about the store becomes stale. I found no official pnpm statement that whole-store deletion is supported or unsupported (see Gaps). Recommendation: never preselect raw deletion of the pnpm store; offer `pnpm store prune` instead.
- **Yarn:** Never treat a project's `.yarn/cache` as disposable, because it may be committed for zero-installs. The global Berry cache and the Yarn classic cache are candidates via `yarn cache clean --mirror` / `yarn cache clean`.
- **Bun:** `~/.bun/install/cache` is likely safe to clear, because macOS clonefile copies are independent of the cache. This is an inference; Bun's page does not explicitly say so. The cache-clearing command (`bun pm cache rm`) was not confirmed on the fetched page.

### Gaps
- I could not retrieve the pnpm store-settings page, so the default macOS store path (commonly `~/Library/pnpm/store`) and the `packageImportMethod` default (clone → hardlink → copy) are **unverified** here. Use `pnpm store path` at runtime instead of hard-coding the path.
- I found no pnpm maintainer statement in GitHub issues on deleting the store directory outright.
- The Yarn classic (v1) cache location and command were not separately verified.
- The Bun cache-clearing command was not verified.

## Rust / Cargo: registry (cache, src, index), git, `cargo-cache`, target dirs

### Takeaway
The Cargo Book says any part of the Cargo home cache *can* be removed and Cargo "will do its best" to restore it. Since Rust 1.88 (June 2025), Cargo automatically garbage-collects unused downloaded data after 3 months and locally recreatable data after 1 month. `bin/` and `.crates*.json` are *not* cache: they record `cargo install`ed tools. `target/` directories are project build output removed by `cargo clean`.

### Cited Findings
- "In theory, you can always remove any part of the cache and Cargo will do its best to restore sources if a crate needs them either by reextracting an archive or checking out a bare repo or by simply redownloading the sources from the web." — [Cargo Book: Cargo Home](https://doc.rust-lang.org/cargo/guide/cargo-home.html)
- **Directory roles:** `registry/index` is "a bare git repository which contains the metadata… of all available crates". `registry/cache` holds downloaded `.crate` archives. `registry/src` holds archives unpacked "where rustc will find the `.rs` files". `git/db` holds bare clones. `git/checkouts` holds checked-out commits from `git/db`. — [Cargo Book: Cargo Home](https://doc.rust-lang.org/cargo/guide/cargo-home.html)
- **What CI should keep:** CI should cache `.crates.toml`, `.crates2.json`, `bin/`, `registry/index/`, `registry/cache/`, `git/db/`. `registry/src` and `git/checkouts` are excluded because the sources would otherwise be stored twice. The `.crates*` files: "Do NOT edit by hand!" — [Cargo Book: Cargo Home](https://doc.rust-lang.org/cargo/guide/cargo-home.html)
- "the internal structure of the Cargo home is not stabilized and may be subject to change at any time." The [cargo-cache](https://crates.io/crates/cargo-cache) crate is mentioned as a tool "to only clear selected parts of the cache". — [Cargo Book: Cargo Home](https://doc.rust-lang.org/cargo/guide/cargo-home.html)
- **Rust 1.88 automatic GC:** Files downloaded from the network are removed when "not accessed in 3 months". Files obtained from the local system are removed when "not accessed in 1 month". GC "will not take place if running offline". — [Rust 1.88.0 announcement](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/)
- **Concurrency/locking:** Cargo uses a shared read lock for builds, a download write lock, and an exclusive lock for cleaning. GC "silently skips… if it is unable to acquire an exclusive lock on the package cache". Last-use tracking lives in `~/.cargo/.global-cache` (SQLite). Target directories are not cleaned by GC. `cargo clean gc` exists for manual runs; the 2023 post called its CLI preliminary. — [Rust Blog: Cargo cache cleaning (Dec 2023)](https://blog.rust-lang.org/2023/12/11/cargo-cache-cleaning/)
- **`cargo clean`:** "With no options, `cargo clean` will delete the entire target directory." The target directory defaults to `target` in the workspace root and can be relocated via `CARGO_TARGET_DIR`/`build.target-dir`. `--dry-run` is supported. — [Cargo Book: cargo clean](https://doc.rust-lang.org/cargo/commands/cargo-clean.html)

### Inferences
- **Safest preselect:** `~/.cargo/registry/src` and `~/.cargo/git/checkouts`, because they are re-extracted locally without network access.
- **Moderate:** `~/.cargo/registry/cache` and `git/db`, which need a re-download.
- **Never touch:** `~/.cargo/bin`, `.crates.toml`, `.crates2.json`, or `config.toml`. Raw deletion bypasses Cargo's package-cache lock, so it should be skipped when `cargo`/`rustc` is running. Since Cargo ≥1.88 auto-GCs anyway, the marginal value of aggressive cleaning is lower.
- **`target/` directories:** These are the largest Rust disk consumers and are fully rebuildable, at the cost of build time. They are good candidates for "inactive project" cleanup.

### Gaps
- I did not verify whether `cargo clean gc` is stabilized as of late 2026. The 2023 post describes it as unstable.

## Gradle and Maven

### Takeaway
Gradle manages `~/.gradle/caches` itself, with retention defaults (30 days for downloads, 7 days for build cache) and cleanup that runs when the daemon stops. Gradle uses cross-process file locks. Manual deletion is not addressed by the docs, so it should only happen with daemons stopped (`gradle --stop`). Maven's `~/.m2/repository` mixes the download cache with locally `mvn install`ed artifacts that have no remote copy, so it is not a pure cache.

### Cited Findings
- **Gradle caches:** `~/.gradle/caches/` contains `build-cache-1`, `jars-9`, `modules-2` (downloaded dependencies), `transforms-3`, and per-version dirs, which "are recreated on first use of that Gradle version". "In most cases they do not require manual management. Gradle creates, uses, and cleans them automatically." — [Gradle: Directory layout](https://docs.gradle.org/current/userguide/directory_layout.html)
- **Gradle retention defaults:** downloaded resources 30 days, created resources 7 days, build cache 7 days, released wrapper distributions 30 days, snapshot wrappers 7 days, daemon logs 14 days. "By default, the cleanup runs in the background when the Gradle daemon is stopped or shut down." Cleanup can be set to `DISABLED` or `ALWAYS` via init scripts. — [Gradle: Directory layout](https://docs.gradle.org/current/userguide/directory_layout.html)
- **Gradle project directories:** project `.gradle` is a "Project-specific cache directory generated by Gradle", and `build` is where Gradle "generates all build artifacts". Both are transient. — [Gradle: Directory layout](https://docs.gradle.org/current/userguide/directory_layout.html)
- **Gradle concurrency:** Gradle uses file-based locking so multiple processes can share the dependency cache. "This concurrent access is only supported if the different Gradle processes can communicate together." Artifacts unused for more than 30 days are deleted, with a scan at most every 24h. Warns against copying `*.lock` or `gc.properties` files. Manual deletion is not addressed. — [Gradle: Dependency caching](https://docs.gradle.org/current/userguide/dependency_caching.html)
- **Gradle daemon:** `gradle --stop` "terminates all Daemon processes started with the same version of Gradle used to execute the command." Daemons stop after being idle 3 hours. — [Gradle: Daemon](https://docs.gradle.org/current/userguide/gradle_daemon.html)
- **Maven:** The local repository "caches remote downloads and contains temporary build artifacts that you have not yet released". The guide says it usually needs no management other than cleaning when disk space runs low. — [Maven: Introduction to Repositories](https://maven.apache.org/guides/introduction/introduction-to-repositories.html). Direct fetch failed; the wording was confirmed via search snippets and the [maven-site source](https://apache.googlesource.com/maven-site/+/6b59d080336ff000e60e5564b3154abdbe83e9b5/content/apt/guides/introduction/introduction-to-repositories.apt). **[verified only via search snippet]**

### Inferences
- **Gradle:**
  - Running daemons are an in-use hazard. `gradle --stop` only stops daemons of its own Gradle version, so an external cleaner should check for any `GradleDaemon` java process.
  - Since Gradle self-cleans, preselecting `~/.gradle/caches` gives limited benefit. Version-specific dirs for Gradle versions no longer used, and `~/.gradle/wrapper/dists` for old versions, are lower-risk targets.
  - Never delete `~/.gradle/gradle.properties` or `init.d`, which hold user config and possibly credentials.
- **Maven:** `~/.m2/repository` is not safe to preselect wholesale. Artifacts installed locally with `mvn install` (for example, internal SNAPSHOTs or artifacts not in any remote repository) may be unrecoverable without rebuilding the source. Never touch `~/.m2/settings.xml`, which holds credentials.

### Gaps
- No Gradle doc explicitly states "stop the daemon before deleting caches". This is inferred from the locking design.
- I did not retrieve the Maven `dependency:purge-local-repository` documentation.

## Go, pip, uv, conda, CocoaPods

### Takeaway
All five have official clean commands. Go's module cache is read-only by design: use `go clean -modcache`, because `rm -rf` fails on it. uv explicitly says it is *never* safe to modify its cache directly; use `uv cache clean`/`prune`, which take a lock. conda's `--force-pkgs-dirs` can break environments linked to the package cache.

### Cited Findings
- **Go module cache:**
  - Default location: "The default location of the module cache is `$GOPATH/pkg/mod`." (`~/go/pkg/mod` when GOPATH is unset.) "The module cache has no maximum size, and the `go` command does not remove its contents automatically." — [Go Modules Reference](https://go.dev/ref/mod)
  - Read-only by design: "The `go` command creates module source files and directories in the cache with read-only permissions… This has the unfortunate side-effect of making the cache difficult to delete with commands like `rm -rf`." `go clean -modcache` "is usually the best way to remove the module cache." — [Go Modules Reference](https://go.dev/ref/mod)
  - Concurrency: "Multiple instances of the `go` command may safely access the same module cache at the same time." — [Go Modules Reference](https://go.dev/ref/mod)
- **Go build cache:** Default location is "a subdirectory named go-build in the standard user cache directory" (on macOS, `~/Library/Caches/go-build`; the OS mapping is an inference). "The go command periodically deletes cached data that has not been used recently." "Running 'go clean -cache' deletes all cached data." "The cache is safe for concurrent invocations of the go command." Other flags: `-testcache` expires test results; `-fuzzcache` removes fuzz-testing files. — [cmd/go](https://pkg.go.dev/cmd/go)
- **pip:**
  - Location and commands: the macOS default is `~/Library/Caches/pip`. pip 26.2+ also respects `XDG_CACHE_HOME` on macOS. `pip cache dir` reports the active path. `pip cache purge` clears "all files from pip's wheel and HTTP caches". — [pip: Caching](https://pip.pypa.io/en/stable/topics/caching/)
  - Structure: the cache structure "is considered to be an implementation detail and may change between any two versions of pip". — [pip: Caching](https://pip.pypa.io/en/stable/topics/caching/)
- **uv:**
  - Location and commands: the default is `$XDG_CACHE_HOME/uv` or `$HOME/.cache/uv`, which also applies on macOS. `uv cache clean` removes all entries but leaves the directory and lock file so other uv processes can share it. `uv cache prune` "removes all _unused_ cache entries" and is safe to run periodically. — [uv: Cache](https://docs.astral.sh/uv/concepts/cache/)
  - Never modify directly: "it's _never_ safe to modify the cache directly (e.g., by removing a file or directory)." — [uv: Cache](https://docs.astral.sh/uv/concepts/cache/)
  - Locking: cache-modifying commands block while other uv commands run, with a default 5-minute timeout (`UV_LOCK_TIMEOUT`). — [uv: Cache](https://docs.astral.sh/uv/concepts/cache/)
- **conda:**
  - Safe options: `--all` removes "index cache, unused cache packages, tarballs, tempfiles, and logfiles". — [conda clean](https://docs.conda.io/projects/conda/en/stable/commands/clean.html)
  - Dangerous option: `--force-pkgs-dirs` removes *all* writable package caches. "This will break environments with packages installed using symlinks back to the package cache." It is not included in `--all`. — [conda clean](https://docs.conda.io/projects/conda/en/stable/commands/clean.html)
  - `--packages` limitation: "Environments not listed by *conda info --envs* are not checked for symlinks back to the package cache." — [conda clean](https://docs.conda.io/projects/conda/en/stable/commands/clean.html)
- **CocoaPods:** `pod cache clean [NAME]`: "Remove the cache for a given pod, or clear the cache completely." `--all`: "Remove all the cached pods without asking." Spec repos live in `~/.cocoapods/repos`. — [CocoaPods: Command-line reference](https://guides.cocoapods.org/terminal/commands.html)

### Inferences
- **Go:**
  - Good candidates via the CLI: `go clean -cache` (build cache, self-trimming, safe concurrently) and `go clean -modcache` (re-downloadable).
  - Don't use `rm`: a cleaner using raw unlink must handle the read-only directory permissions. Using `go clean` avoids this.
  - Offline hazard: `-modcache` hurts offline or vendored-less builds until modules are re-downloaded.
- **pip:** `pip cache purge` is a strong preselect candidate. It only affects download and wheel-build speed.
- **uv:** Use only `uv cache prune` (preferred) or `uv cache clean`. Never delete `~/.cache/uv` directly, per uv's explicit statement. The uv page notes that environments link files from the cache. It does not say that deleting the cache breaks existing venvs, but raw deletion is officially unsupported.
- **conda:** Preselect only `conda clean --all` (or `--tarballs --index-cache`). Never run `--force-pkgs-dirs`, and never delete `pkgs/` directly.
- **CocoaPods:** `~/Library/Caches/CocoaPods` via `pod cache clean --all` is a reasonable candidate. The cache path is not confirmed on the fetched page.

### Gaps
- The Go docs as fetched did not state the build-cache trim interval. The often-cited "5 days" was not confirmed.
- The CocoaPods cache path and the safety of removing `~/.cocoapods/repos` (trunk is CDN-based in modern CocoaPods) were not confirmed from official docs.
- The conda default macOS `pkgs` location depends on the install (Miniconda, Anaconda, Miniforge) and was not researched.

## Browser test binaries (Playwright, Puppeteer, Cypress) and Docker Desktop

### Takeaway
The Playwright and Cypress browser and binary caches are re-downloadable and have official uninstall or prune commands. Playwright already auto-GCs unused browser versions. Docker Desktop space must be reclaimed through `docker system prune`, which does not remove volumes by default, or through Docker's own settings UI. `Docker.raw` holds all images, containers and volumes, and must not be touched by a cleaner.

### Cited Findings
- **Playwright:**
  - Location: macOS cache is `~/Library/Caches/ms-playwright`, about 180–280 MB per browser. — [Playwright: Browsers](https://playwright.dev/docs/browsers)
  - Auto-GC: "Playwright keeps track of the clients that use its browsers." When a version is no longer needed, "that version is deleted from the system". This can be disabled with `PLAYWRIGHT_SKIP_BROWSER_GC=1`. — [Playwright: Browsers](https://playwright.dev/docs/browsers)
  - Uninstall: `npx playwright uninstall` removes the current installation's browsers. `--all` removes browsers for all installations. — [Playwright: Browsers](https://playwright.dev/docs/browsers)
- **Puppeteer:** Since v19.0.0, Puppeteer "stores browsers in `~/.cache/puppeteer` to globally cache browsers between installation". The location is configurable via `cacheDirectory`. — [Puppeteer: Configuration](https://pptr.dev/guides/configuration)
- **Cypress:**
  - Location: macOS binary cache is `~/Library/Caches/Cypress`, overridable with `CYPRESS_CACHE_FOLDER`. — [Cypress: Advanced installation](https://docs.cypress.io/app/references/advanced-installation)
  - Commands: `cypress cache prune` "Deletes every cached binary except the version currently in use". `cypress cache clear` "Deletes all cached Cypress binaries". — [Cypress: Advanced installation](https://docs.cypress.io/app/references/advanced-installation)
  - App data: app data lives separately in `~/Library/Application Support/Cypress`. — [Cypress: Advanced installation](https://docs.cypress.io/app/references/advanced-installation)
- **Docker Desktop disk image:**
  - Location: "Docker Desktop stores Linux containers and images in a single, large 'disk image' file", typically `~/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw`. — [Docker Desktop Mac FAQ](https://docs.docker.com/desktop/troubleshoot-and-support/faqs/macfaqs/)
  - Reported size: many tools report the maximum file size, not the actual size. — [Docker Desktop Mac FAQ](https://docs.docker.com/desktop/troubleshoot-and-support/faqs/macfaqs/)
  - Moving it: "Do not move the file directly in Finder as this can cause Docker Desktop to lose track of the file." — [Docker Desktop Mac FAQ](https://docs.docker.com/desktop/troubleshoot-and-support/faqs/macfaqs/)
  - Reclaiming space: reducing the disk size deletes the image file, and "all containers and images are lost". "Space is only freed when images are deleted." — [Docker Desktop Mac FAQ](https://docs.docker.com/desktop/troubleshoot-and-support/faqs/macfaqs/)
- **`docker system prune`:** By default it removes stopped containers, unused networks, dangling images, and unused build cache. `--all` also removes unused (not just dangling) images. `--volumes` prunes anonymous volumes. Volumes are not removed by default, to prevent "important data from being deleted". — [docker system prune](https://docs.docker.com/reference/cli/docker/system/prune/)
- **Docker "Clean up data":** "A disk image reset destroys all Docker containers and images local to the machine, preserving all settings." — [Docker Desktop: Troubleshoot](https://docs.docker.com/desktop/troubleshoot-and-support/troubleshoot/)

### Inferences
- **Playwright:** `~/Library/Caches/ms-playwright` can be offered, but because of auto-GC, whatever remains is likely in use. Deleting it forces `npx playwright install` before tests run.
- **Cypress:** `cypress cache prune` is the safest preselect. It keeps the version in use.
- **Docker:**
  - Never delete or edit `Docker.raw` or anything under `~/Library/Containers/com.docker.docker`. It contains named volumes, which may hold databases and other unrecoverable user data. Docker's docs warn even against moving it in Finder. They do not literally say "don't delete Docker.raw"; that is an inference from these statements.
  - At most, offer `docker system prune` (without `--volumes`, and arguably without `--all`) as an opt-in. It needs the Docker daemon running, and removing stopped containers can discard state the user wanted.
  - Shrinking `Docker.raw` on disk after a prune can lag "a few minutes".

### Gaps
- No Puppeteer-official command for clearing old browsers was confirmed on the fetched page.
- Docker docs do not explicitly say that deleting `Docker.raw` destroys volumes, though the disk image is the only storage for the Linux VM.

## In-project `node_modules` / `target` / `build` directories for inactive projects

### Takeaway
These are documented as generated, transient outputs: Cargo `target` via `cargo clean`, and Gradle `build`/`.gradle` as "transient files". They are rebuildable from source plus lockfile. The cost is a full re-install or rebuild, which needs network access for dependency downloads. Deletion is reasonable for *inactive* projects, but the cleaner must confirm the directory really is generated output.

### Cited Findings
- `cargo clean` "will delete the entire target directory". The target directory may be relocated by `CARGO_TARGET_DIR`/`build.target-dir`. — [Cargo Book: cargo clean](https://doc.rust-lang.org/cargo/commands/cargo-clean.html)
- Gradle's `build` directory is where Gradle "generates all build artifacts". Project `.gradle` is a "Project-specific cache directory generated by Gradle". Both hold "transient files Gradle uses to support features like incremental builds". — [Gradle: Directory layout](https://docs.gradle.org/current/userguide/directory_layout.html)
- Bun re-populates missing packages in `node_modules` from the cache or registry on `bun install`. — [Bun docs: install cache](https://bun.sh/docs/install/cache)
- Yarn zero-install projects commit `.yarn/cache`, so this directory looks like a cache but is source-controlled. — [Yarn: Caching](https://yarnpkg.com/features/caching)

### Inferences
- **`node_modules`:** Regenerable via `npm ci`/`pnpm install`/`yarn`/`bun install` only if a lockfile exists. Without one, a reinstall may resolve different versions. A cleaner should also skip `node_modules` containing hand-patched packages (without patch-package) or locally linked packages.
- **Generic `build` names:** Directories named `build`/`dist`/`out` are ambiguous and can be user content or committed. Only treat them as generated when a matching build manifest exists (`build.gradle`, `Cargo.toml` + `target`, `package.json` + `node_modules`) and, ideally, when they are git-ignored. Never preselect directories tracked by git.
- **Inactivity:** Inactivity should be judged on project-level mtimes or last git activity, not on the artifact directory itself, which a background process may touch.
- **Running processes:** Skip when a dev server, build tool, or IDE has the project open, because deleting `node_modules` under a running dev server can crash it.

### Gaps
- None of the npm, pnpm, or Yarn official docs I fetched contain explicit guidance on deleting `node_modules` for inactive projects. I found no official source; the guidance above is inference.
- Tools like `npkill` and `cargo-sweep` exist but were not researched.
