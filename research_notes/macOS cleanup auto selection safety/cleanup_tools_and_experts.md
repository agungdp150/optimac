# Default-selection behaviour of Mac cleanup tools and expert guidance

Research date: 2026-11-13. About 18 search and fetch calls. Primary sources are vendor help pages, GitHub READMEs and eclecticlight.co. Macworld, TidBITS, Six Colors, iMore, 9to5Mac and AppleInsider were not fetched directly; see Gaps.

## MacPaw CleanMyMac (current) and CleanMyMac X: what is auto-selected, the Safety Database, unchecked categories, incidents

### Takeaway
In current CleanMyMac, every Smart Care tile is selected by default. Cleanup (system junk, mail attachments, Trash bins) and Protection (malware) auto-select their findings. "My Clutter" covers only duplicates in Downloads. MacPaw says personal files are never removed unless the user chooses them. The "Safety Database" is a proprietary rule set, and MacPaw's description of it is marketing-level only. User reports describe Photos-library losses with older versions (CMM2, CMM3, CMM X), but none is confirmed.

### Cited Findings
- "By default, CleanMyMac selects all Smart Care tasks." You can clear the checkmark in the top-left corner of a tile to skip it. — [MacPaw: Smart Care](https://macpaw.com/support/cleanmymac/knowledgebase/smart-care)
- Smart Care tiles: **Cleanup** scans for system junk ("log files, cache files, and broken login items"), mail attachments and trash bins, and "automatically selects items for removal". **Protection** runs a Quick Scan for malware and auto-selects threats. **Performance** suggests maintenance scripts and Flush DNS Cache. **Applications** recommends app updates, excluding system apps and macOS updates. **My Clutter** "scans for duplicate files in your Downloads folder". — [MacPaw: Smart Care](https://macpaw.com/support/cleanmymac/knowledgebase/smart-care)
- MacPaw: "CleanMyMac doesn't delete any user files or app data unless you choose to." It also says a "Safety Database, built over years of experience, prevents cleaning errors and protects user data". — [MacPaw: Smart Care](https://macpaw.com/support/cleanmymac/knowledgebase/smart-care)
- Safety Database: "It contains detailed rules and exceptions tailored to various macOS versions and apps", and it is updated continuously. By default the Cleanup module "only offers to remove automatically generated system or app-related data", and "no personal files are removed unless you choose to delete them". — [MacPaw: CleanMyMac safety and reliability](https://macpaw.com/support/cleanmymac/knowledgebase/cleanmymac-safety) (vendor-only claim, with no technical detail published)
- Ignore lists let users exclude items from results. They exist for Cleanup (System Junk, **AI Clutter**, Mail Attachments), Protection (Malware Removal), Applications (Uninstaller, Updater) and My Clutter (Duplicates, Similar Images, Downloads). — [MacPaw: CleanMyMac safety](https://macpaw.com/support/cleanmymac/knowledgebase/cleanmymac-safety)
- There is an older CleanMyMac X version of the safety page. — [MacPaw: CleanMyMac X Safety and Reliability](https://macpaw.com/support/cleanmymac-x/knowledgebase/cleanmymac-safety) (listed in search results but not fetched)
- Forbes (Oct 2024) reports that the new CleanMyMac's default scan is "Balanced Mode". This setting controls scan depth, not which items are selected. — [Forbes, 2024-10-16](https://www.forbes.com/sites/marksparrow/2024/10/16/the-new-cleanmymac-update-will-keep-your-mac-fit-and-health/) (seen in search snippet only)
- Incidents, all anonymous forum posts with unconfirmed causes and no MacPaw response found:
  - CleanMyMac 2 flagged a new Photos library as a "large & old file". The user deleted it and Photos could no longer find the library. — [Apple Community thread 7006767](https://discussions.apple.com/thread/7006767) (circa 2015)
  - The CleanMyMac 3 Photos optimisation left a library unavailable or corrupt. — [Apple Community thread 8411740](https://discussions.apple.com/thread/8411740) (circa 2018)
  - A user says a CleanMyMac X duplicate scan removed non-duplicate photos, with nothing left in Trash or Recently Deleted. Confounders: a migrated iPhoto library and low disk space. — [Apple Community thread 255229724](https://discussions.apple.com/thread/255229724)
  - Another CleanMyMac X report. — [Apple Community thread 250070606](https://discussions.apple.com/thread/250070606)

### Inferences
- MacPaw's current model has two tiers. Data that apps and the system regenerate (caches, logs, mail attachment copies, Trash) is auto-selected. User-content features (Large & Old Files, Similar Images, most duplicates) need manual review. The one exception is Downloads duplicates, which Smart Care surfaces in My Clutter.
- The documented incidents involve user-content modules (Large & Old, duplicates, Photos optimisation), not cache cleanup. This supports keeping anything inside Photos libraries or other user libraries out of any auto-selected tier.

### Gaps
- I found no current MacPaw page that lists default checkbox states inside System Junk subcategories, for example language files, iOS backups, iOS software updates, Xcode junk, or "Universal binaries". Whether My Clutter's Downloads duplicates are pre-checked is also not documented.
- MacPaw has not published the contents or methodology of the Safety Database.
- I found no official MacPaw incident acknowledgement.

## DaisyDisk: auto-selection, hidden and purgeable space, the Collector

### Takeaway
DaisyDisk auto-selects nothing. It is a visualiser where the user drags items into a "Collector" staging area, and nothing is deleted until the user clicks Delete. It shows "hidden space" and reports "purgeable space" (mostly local Time Machine snapshots) as one macro item. That item can be force-purged through the Collector, but only in the non-App Store edition.

### Cited Findings
- Hidden space includes purgeable space, which is "mostly local snapshots of Time Machine, and also swap files, sleep images and other temporary system files". — [DaisyDisk guide: hidden space](https://daisydiskapp.com/guide/hidden-space)
- "Starting from macOS High Sierra and APFS, the purgeable space is located outside of any scannable area, even with raised permissions. Instead, DaisyDisk provides a macro-level view and deletion of the purgeable space." Purging is triggered by dragging the item to the Collector and can take 2 to 10 minutes. It is not available in the Mac App Store edition. — [DaisyDisk guide: purgeable space](https://daisydiskapp.com/guide/purgeable-space)
- "Files in the Collector remain intact until you click Delete." Folders such as /System, /Library and the current user's home folder are "not meant to be deleted, so the Collector will not accept them." — [DaisyDisk guide: deleting files](https://daisydiskapp.com/guide/deleting-files) (from search-result excerpts of the guide)
- DaisyDisk deletes permanently rather than moving items to Trash, "so that the disk space is actually recovered". — Source is a third-party mirror in search results; I could not verify it against the official guide. Treat it as unconfirmed. See the [DaisyDisk guide v3: deleting files](https://daisydiskapp.com/guide/3/en/DeletingFiles).
- Purgeable-space display and reclaim were added in DaisyDisk 4.10. — [DaisyDisk releases](https://daisydiskapp.com/releases)
- Howard Oakley (Sep 2026): "DaisyDisk has always been good for me." He notes that analysis tools need to build their own caches, so when the disk is completely full "Storage would be your only option". — [eclecticlight: Storage crisis management, 2026-09-06](https://eclecticlight.co/2026/09/06/last-week-on-my-mac-storage-crisis-management/) (comment reply)

### Inferences
- DaisyDisk represents the "never auto-select, the user stages explicitly" end of the spectrum. Its Collector is a staging and preview area, similar to a dry-run list.
- It refuses to accept system roots and the home folder itself. That is a hard-protection list, separate from selection defaults.

### Gaps
- I did not confirm whether the current version (2025–2026) has any "suggested" or recommended items.

## OnyX (Titanium Software): default checkboxes in Maintenance > Cleaning, author warnings

### Takeaway
I could not confirm the current default checkbox state of OnyX's Cleaning options from primary sources. The official page lists capabilities and warns only about using the correct OS version. Third-party and older sources say user-cache cleaning is indiscriminate and should be used only to fix misbehaving apps.

### Cited Findings
- Official page: OnyX verifies system file structure, runs cleaning and maintenance tasks, deletes caches, removes "certain problematic files and folders" and rebuilds databases and indexes. Its only caution is: "Use the correct version, and don't try to use a version intended for an earlier operating system." The current release is reported as OnyX 5.1.2 (2026-11-02) for Apple-silicon Macs running macOS 27. Older OS versions are discontinued. — [Titanium Software: OnyX](https://www.titanium-software.fr/en/onyx.html) (version details as returned by fetch; verify before quoting)
- An Apple Community user guide (old, undated, likely about 2010s) describes user caches as "miscellaneous cache files created by different applications in your home folder. If an application is acting up, you may want to delete them; if not, don't use this command." — [Apple Community: Clearing caches with OnyX](https://discussions.apple.com/docs/DOC-2258)
- A 2026 third-party review says OnyX's "clean all user caches" option "will clean all user caches — including ones you'd rather keep", for example Xcode DerivedData and the Spotify offline cache. — [sweepformac.com OnyX review](https://www.sweepformac.com/guides/onyx-review/) (single source; a vendor of a competing tool)
- Circa 2014, "Cleaning > User > Dock Icons" was reportedly checked by default. — [FineTunedMac forum](https://www.finetunedmac.com/forums/ubbthreads.php?ubb=showflat&Number=30150) (old, may not apply to 5.x)

### Inferences
- OnyX is a power-user tool that frames cleaning as troubleshooting, not routine hygiene. That matches Oakley's position below.

### Gaps
- The current (5.x) default checkboxes in Maintenance > Cleaning are unknown. Confirming them needs a screenshot of the running app or the in-app help.
- I found no current author text on the side effects of cleaning.

## Open-source cleaners: Mole, Pearcleaner, mac-cleanup-py, Lemon

### Takeaway
Mole is the most explicit about safety. It has a "known-safe" default clean set, a persistent whitelist, `--dry-run` on every destructive command, refusal when it cannot prove an item is safe, an operations log, and recency-based unselection of build artifacts (anything touched in the last 7 days is unselected). Its uninstaller keeps personal data by default. mac-cleanup-py runs broad modules, including iOS backups and Trash, with only a dry-run flag and a module configurator. Pearcleaner's and Lemon's READMEs do not document their selection defaults.

### Cited Findings
**Mole (tw93/Mole)** — [README](https://raw.githubusercontent.com/tw93/Mole/main/README.md)
- `mo clean` "reviews known-safe caches, logs, temporary files, developer artifacts, and leftovers from apps that are no longer installed." The sample output covers user app cache, user app logs, Trash, app caches, Safari and Chrome caches, and npm and pnpm caches.
- "It empties `~/.Trash` by default", but not volume Trashes (`/Volumes/*/.Trashes`).
- `mo clean --whitelist` protects selected caches or items, and the choices persist in `~/.config/mole/whitelist`. "If the whitelist file cannot be read, `clean`, `purge`, and `optimize` stop instead of running without it." Custom whitelists replace the optional defaults, but "built-in safety protections still apply."
- `--dry-run` works on `clean`, `uninstall`, `optimize`, `purge`, `installer` and `slim`, and the README tells users to run it first.
- "When Mole cannot prove an item is safe to change, it skips or refuses it." Mole "validates paths, protects shared and system-owned locations".
- `uninstall`, `analyze` and `slim` move items to Trash by default. `clean`, `purge` and `installer` delete permanently. Operations are logged to `~/Library/Logs/mole/operations.log`.
- `mo purge` (node_modules, target, build, dist and similar): "Artifacts with file activity in the last 7 days, or activity Mole cannot verify, are unselected by default." It protects directories that contain deployment keypairs, nested Git repos or Git-tracked files.
- Uninstall: "Personal data, including settings and plugins, is kept by default". `--with-data` pre-selects that data, except browser profiles and App Group containers. Shared data is kept "when another installed copy still uses it."
- `mo installer` scans Downloads and iCloud for installer files. So a user folder is in scope, but only for one narrow file type.

**Pearcleaner (alienator88)** — [README](https://raw.githubusercontent.com/alienator88/Pearcleaner/main/README.md)
- Pearcleaner is a "free, source-available and fair-code licensed Mac app cleaner". It offers App Uninstall, Orphaned File Search, Development Environment Manager, File Search, and a Sentinel Monitor that runs "automatic cleanup when apps hit Trash". It needs Full Disk Access and a privileged helper for system folders.
- The README does not document pre-selection, protections or Trash behaviour. The project is marked "On Hold", with development effectively stopped since the end of 2025 (version 5.4.3).

**mac-cleanup-py** — [README](https://raw.githubusercontent.com/mac-cleanup/mac-cleanup-py/main/README.md)
- It empties Trash, deletes logs, clears caches, and has modules for browser caches, dev package caches (npm, yarn, pnpm, Go, Gradle), Xcode, Docker, and system caches and logs. All of these are listed under "Default Modules".
- Modules that touch valuable data: `ios_backups` ("Removes iOS Device Backups"), `trash` (all mounted volumes), and `xcode_simulators` (resets simulators).
- Flags: `-n, --dry-run` ("Run without deleting stuff"), `-v` lists the folders, `-c, --configure` opens a module configuration screen, and `-f, --force` means "Accept all warnings". The README describes no user-data safeguards.

**Lemon (Tencent/lemon-cleaner)** — [README](https://raw.githubusercontent.com/Tencent/lemon-cleaner/master/README.md)
- Lemon offers per-app scan plans (Xcode, Sketch), large files over 50 MB, duplicates and similar photos (in user-chosen folders), browser privacy data and uninstall residue. The README documents no default selections or whitelist, and its maintenance status is not stated.

### Inferences
- Mole's design gives a ready-made pattern for tiers. Auto-include rebuildable, known-safe data. Unselect anything recently modified or unverifiable. Keep personal data out unless the user opts in. Refuse rather than guess. Prefer Trash for user-facing deletions.
- mac-cleanup-py shows a risk: iOS backups and all-volume Trash sit in a "default" module set, with only a CLI dry-run as protection.

### Gaps
- I did not read source code, only READMEs, so the exact Mole default selection lists for `clean` and `uninstall` are not confirmed. Mole's own whitelist defaults (the "optional defaults") are not enumerated in the README.
- I did not cover mac-cleanup-sh separately. I did not research AppCleaner (FreeMacSoft) defaults.
- Pearcleaner's related-file matching algorithm and pre-check behaviour are not documented in the README. Its source would need to be read.

## Expert opinion: Howard Oakley and Mac press on cache cleaning and cleaner apps

### Takeaway
Oakley's position is consistent across his writing. Caches are a "double-edged sword" that speed up the UI. macOS (CacheDelete and the `deleted` service) purges them itself when space is needed. Deleting them frees less space than expected and can slow things down, for example Finder Gallery view. Flushing caches is a troubleshooting step, not routine maintenance. He prefers developer-documented leftovers to heuristic cleaners. Secondary sources broadly agree that cache clearing is low-value and that uninstall leftovers, large or old files and installers are where real space is recovered.

### Cited Findings
- Disk caches are "optional extras, and when macOS needs to find additional free space they're candidates for deletion, or purging". The `deleted` service manages this and can also purge local Time Machine snapshots. — [eclecticlight: Caches and purging in macOS Tahoe, 2026-09-02](https://eclecticlight.co/2026/09/02/caches-and-purging-in-macos-tahoe/)
- "There are distinct trade-offs from purging caches." Deleting QuickLook caches would make Finder "much slower in Gallery mode". The caches in question "aren't old, normally, they're the current ones in active use". — [eclecticlight, 2026-09-02](https://eclecticlight.co/2026/09/02/caches-and-purging-in-macos-tahoe/) (comment replies)
- On flushing caches via Safe mode: deleting caches "is only likely to be able to provide a maximum, and will invariably achieve considerably less", and "may not free any significant storage space". It is worth trying only after deleting snapshots and evicting iCloud files. — [eclecticlight: Will flushing caches free up disk space?, 2023-09-22](https://eclecticlight.co/2023/09/22/will-flushing-caches-free-up-disk-space/)
- The Safe-mode cache flush is useful after macOS-update glitches but shouldn't be "a routine maintenance procedure". — [eclecticlight: Safe mode, caches and CacheDelete, 2023-05-03](https://eclecticlight.co/2023/05/03/safe-mode-caches-and-cachedelete/) (search-snippet paraphrase)
- "You also shouldn't need to worry about /private/var/folders, which is controlled by macOS not the app, as are caches." "Why should you have to use another app to try to clean up removed apps?" He wants developers to document files installed outside the bundle. Most leftovers "just waste disk space", but leftover security software can "cause serious instability". — [eclecticlight: Can we make cleaning up our Macs simpler?, 2025-01-26](https://eclecticlight.co/2025/01/26/last-week-on-my-mac-can-we-make-cleaning-up-our-macs-simpler/)
- On one cleaning utility he examined: "I think it adopts an inherently dangerous approach, and some of the information given is either misleading or incorrect". He criticises its "old scripted workaround to perform privileged operations, rather than doing it properly with a privileged helper app." — [eclecticlight: Storage crisis management, 2026-09-06](https://eclecticlight.co/2026/09/06/last-week-on-my-mac-storage-crisis-management/) (comment reply; the utility is not named in the extract)
- Correction: the phrase "care needs to be taken before committing to any deletion" on that page was written by a reader, not Oakley. — [same page](https://eclecticlight.co/2026/09/06/last-week-on-my-mac-storage-crisis-management/)
- Secondary sources, lower weight:
  - A MacRumors forum consensus says there is no point emptying caches for space because the OS refills them. — [MacRumors: Is CleanMyMac actually legit or snake oil?](https://forums.macrumors.com/threads/is-cleanmymac-actually-legit-or-snake-oil.2476091/) (from search snippet)
  - A review says Oakley "has argued against cleaner apps since 2016". It also says the durable space wins are app leftovers, duplicates, old installers and large forgotten files, while most reported "junk" is cache. — [coreguard.app: Is CleanMyMac worth it?](https://coreguard.app/insights/is-cleanmymac-worth-it/) (competitor content, from search snippet)
  - Macworld has a 2026 comparison of 14 cleaners. — [Macworld: Best Mac cleaner software 2026](https://www.macworld.com/article/673271/best-mac-cleaner-tested-compared.html) (not fetched; contents unverified)

### Inferences
- The expert consensus supports a tier design where generic cache clearing is a low-value, optional tier. It should not be a headline "recommended" item, and should not be presented as a performance boost. Uninstall leftovers, old installers and large items the user reviews give more durable space gains.
- Oakley's ordering for space recovery (snapshots, then iCloud eviction, then cache flush as a last resort) suggests that purgeable or snapshot space deserves a separate, explained category.

### Gaps
- I did not fetch TidBITS, Six Colors, iMore, 9to5Mac or AppleInsider guidance, so no direct citations exist for them. The AppleInsider 2024 CleanMyMac X piece appeared in results but is a sponsored-style article ([AppleInsider, 2024-07-29](https://appleinsider.com/articles/24/07/29/keep-your-mac-running-smoothly-with-cleanmymac-x)).
- Search snippets attribute a quote to Oakley, "as those apps usually don't inform you clearly what they're going to remove, I don't think they're sufficiently informative as to be trustworthy", but I could not verify which article it comes from. Do not quote it without verification.
- I found no Apple Support guidance in this pass.

## Auto-selection in user folders and duplicates (Gemini 2 and others)

### Takeaway
MacPaw's Gemini 2 is the clearest case of a reputable tool auto-selecting user files. When scanning Home, Smart Selection auto-marks duplicates in **Downloads and Desktop** but **not Documents**. It never auto-selects "similar" files, and it moves removals to Trash. Gemini Classic kept the oldest copy and selected newer copies. CleanMyMac's Smart Care also touches only Downloads duplicates. No reviewed tool auto-selects Documents.

### Cited Findings
- Gemini 2: "When you scan the Home folder, Smart Selection marks duplicates in Downloads and Desktop for removal automatically. However, duplicates in Documents will not be selected." Similar files are never offered for automatic removal and need the user's review. Smart Cleanup moves selected duplicates to the Trash. Rules can be viewed, enabled, disabled and restored in Preferences > Smart Selection, and Gemini may suggest new rules learned from user choices (for example, "select newer files by default"). The page says originals stay on the Mac but does not say how the original is chosen. — [MacPaw: All about Smart Cleanup (Gemini 2)](https://macpaw.com/support/gemini/knowledgebase/gemini-2-smart-selection)
- Gemini Classic: it "selects the newer (or newest) version of the file for removal… Gemini keeps the older (oldest) copy". — [MacPaw: How does Automatic Selection work? (Gemini Classic)](https://macpaw.com/support/gemini-classic/knowledgebase/gemini-automatic-selection-work) (older product)
- MacPaw marketing says the default Smart Cleanup removes only "100% safe duplicates" and, for Photos, saves "the best version of each photo". — From search snippets of MacPaw pages (vendor-only claim)
- CleanMyMac Smart Care's My Clutter scans only "duplicate files in your Downloads folder". — [MacPaw: Smart Care](https://macpaw.com/support/cleanmymac/knowledgebase/smart-care)
- Mole's `installer` command scans Downloads and iCloud, but only for installer files. — [Mole README](https://raw.githubusercontent.com/tw93/Mole/main/README.md)

### Inferences
- An industry pattern emerges. Downloads, and in Gemini's case Desktop, are treated as "transient" user folders where duplicate copies may be auto-selected. Documents is treated as protected. Similar or near-duplicate items always need human review. Deletion of user content goes to Trash, so it can be undone.
- Even the most aggressive defaults found (Gemini 2) apply only to exact duplicates where another copy remains. Nobody auto-selects unique files in user folders.

### Gaps
- I found no public statement of Gemini 2's precise rule for which copy counts as the "original" (oldest, shortest path, or location-based).
- I did not check whether CleanMyMac's Downloads duplicates in Smart Care are pre-checked inside the tile.
