# Finding and removing duplicate files safely on macOS (APFS, iCloud Drive, File Provider)

Scope: how to detect true duplicates on macOS, what deleting a copy actually frees, and which copy an auto-select feature should keep. Sources were collected 2026-11-13. Local SDK headers and man pages come from this Mac (Darwin 25.6 / macOS 26 Command Line Tools SDK) and are cited as "local SDK header" or "local man page". The getattrlist man page is also mirrored online at keith.github.io.

## 1. APFS clones: detection and "space freed"

### Takeaway
Two byte-identical files on APFS can share some or all of their data blocks (clones). Deleting one of them frees only the blocks that no other file or snapshot still uses, which can be close to zero. The correct per-file figure is `ATTR_CMNEXT_PRIVATESIZE` from `getattrlist`. Pure clones can be grouped by `ATTR_CMNEXT_CLONEID`, and `EF_MAY_SHARE_BLOCKS` / `EF_SHARES_ALL_BLOCKS` flag files that share blocks. Finder's "On Disk" figure ignores sharing and overstates what deletion would free.

### Cited Findings
- **How clones are created.** A Finder duplicate tries to create a clone rather than a full copy. Clones can only be made within one volume, and copying a clone to another volume produces "a complete copy, not a clone." — [Oakley, APFS: Files and clones](https://eclecticlight.co/2024/03/20/apfs-files-and-clones/)
- **What a clone shares.** A clone gets its own inode but initially "shares the same data blocks and extended attributes" with the original. "There's a flag in the file's attributes to indicate that cloning has taken place." — [Oakley, APFS: Files and clones](https://eclecticlight.co/2024/03/20/apfs-files-and-clones/)
- **Clones drift apart.** As the files are edited, "they start to drift apart." Changed data goes to new blocks, so the extents overlap only partly, and the files "may cease to share any common data." This means partial sharing exists between files whose bytes no longer match. — [Oakley, APFS: Files and clones](https://eclecticlight.co/2024/03/20/apfs-files-and-clones/)
- **Deleting a clone.** "Delete one of the clones and those components remain, as they're still required by the other clone." Blocks referenced by a snapshot are likewise "retained with the snapshot until that's deleted." — [Oakley, APFS: Files and clones](https://eclecticlight.co/2024/03/20/apfs-files-and-clones/)
- **Oakley says hashing cannot identify clone pairs.** He writes that "there's no direct way to tell which are clones of one another." The only method he knows is a checksum or hash search, which "will only identify identical clones that haven't changed." This is contradicted by Apple's `ATTR_CMNEXT_CLONEID` documentation in the next bullet; Oakley may simply not have covered that attribute. — [Oakley, APFS: Files and clones](https://eclecticlight.co/2024/03/20/apfs-files-and-clones/)
- **Apple's `getattrlist` documentation** describes these extended common attributes:
  - `ATTR_CMNEXT_PRIVATESIZE`: "the number of bytes that are not trapped inside a clone or snapshot, and which would be freed immediately if the file were deleted."
  - `ATTR_CMNEXT_CLONEID`: "uniquely identifies the data stream associated with the file system object. Useful for finding which files are pure clones of each other (as they will have the same clone-id)."
  - `ATTR_CMNEXT_CLONE_REFCNT`: "the number of full clones (each shares all of its blocks with this file)."
  - Flags in `ATTR_CMNEXT_EXT_FLAGS`: `EF_MAY_SHARE_BLOCKS` ("may be a clone of another file"), `EF_SHARES_ALL_BLOCKS` ("it is a full clone of another file"; it also implies `EF_MAY_SHARE_BLOCKS`), `EF_IS_SPARSE`, `EF_IS_PURGEABLE`, `EF_NO_XATTRS` ("Useful for avoiding a call to listxattr()"), `EF_IS_SYNC_ROOT` and `EF_IS_SYNTHETIC`.

  Sources: [getattrlist(2) mirror](https://keith.github.io/xcode-man-pages/getattrlist.2.html); also confirmed in the local `man getattrlist` on macOS 26.
- **Finder "On Disk" is misleading for clones.** The Get Info "On Disk" figure "isn't reduced" for blocks shared with another file. In Oakley's test, a clone of a 107.41 GB VM showed 54.57 GB on disk in Get Info, while Disk Utility showed only 0.74 GB of extra space actually used. Oakley explains that working out which extents are shared is too slow to include in on-disk size calculations, and that Finder doesn't reveal the "has been cloned" flag. — [Oakley, When the Finder's size 'on disk' is misleading (Sep 2026)](https://eclecticlight.co/2026/09/05/when-the-finders-size-on-disk-is-misleading/)
- **Precize.** Precize shows whether a file "has at some time been cloned," its link/reference count, and its volfs path (volume ID and inode). — [Oakley, APFS: Files and clones](https://eclecticlight.co/2024/03/20/apfs-files-and-clones/); see also search-result summaries of [eclecticlight tag: clone](https://eclecticlight.co/tag/clone) and [Last Week on My Mac: Attack of the Clones](https://eclecticlight.co/2024/03/24/last-week-on-my-mac-attack-of-the-clones/), which I did not read in full.
- **Cross-platform tools treat clones as an action, not a reason to skip files.**
  - jdupes `-B` performs "copy-on-write (reflink/clone) deduplication." — [jdupes README](https://codeberg.org/jbruchon/jdupes)
  - fclones `dedupe` "does not remove any files, but deduplicates file data by using native copy-on-write capabilities of the file system (reflink)." — [fclones README](https://github.com/pkolaczk/fclones)
  - rmlint has `clone`/`reflink` handlers and `--is-reflink`, documented in terms of Linux FIDEDUPERANGE/fiemap. Its manual does not mention APFS or `clonefile`. — [rmlint(1)](https://rmlint.readthedocs.io/en/latest/rmlint.1.html)
  - None of these READMEs document APFS clone detection or clone-aware "space freed" reporting.

### Inferences
- **Report reclaimable space as the sum of `ATTR_CMNEXT_PRIVATESIZE`** over the files you would delete, not the sum of logical size or `st_blocks`.
- **Sum PRIVATESIZE with care.** If A and B are full clones of each other and you delete both, each file's PRIVATESIZE (about 0) undercounts the space freed. Group by `CLONEID` first: deleting all members of a clone group frees the shared blocks only if no member is kept and no snapshot holds them.
- **Handle full clone groups explicitly.** Files with the same `CLONEID` (or the flag `EF_SHARES_ALL_BLOCKS`) are pure clones, and deleting all but one frees roughly nothing. A cleanup app should either hide such groups or label them "no space reclaimed (APFS clone)".
- **Label partial sharing.** For `EF_MAY_SHARE_BLOCKS` without a matching CLONEID, show the PRIVATESIZE figure and mark it "may be lower due to shared blocks".
- **Snapshots trap space too.** Local Time Machine and APFS snapshots mean even non-cloned files may not free space immediately. PRIVATESIZE accounts for snapshots according to Apple's wording ("not trapped inside a clone or snapshot").
- **Clone deduplication is a second strategy** that keeps every path while reclaiming the space: replace a copy with `clonefile()` of the keeper. jdupes and fclones offer this. Whether they call `clonefile` on macOS was not confirmed in their READMEs.

### Gaps
- I found no Apple document stating which macOS version introduced `ATTR_CMNEXT_PRIVATESIZE`, `CLONEID` and `CLONE_REFCNT`. They are present in the macOS 26 SDK man page.
- I found no source on how commercial Mac duplicate finders (Gemini 2, PhotoSweeper, Nektony) report "space freed" for clones. No vendor documentation addresses it.
- `F_LOG2PHYS`/`F_LOG2PHYS_EXT` (fcntl) could map logical offsets to physical blocks to compare extents. I found no authoritative source describing this use for clone detection, so it is unverified.
- I could not confirm whether PRIVATESIZE is cheap to query in bulk (for example through `getattrlistbulk`) on large trees.

## 2. Hard links

### Takeaway
Hard links are one inode reached by several paths, so deleting one path frees nothing while another link remains. Every mainstream tool detects them by device and inode (or `st_nlink`) and by default excludes them from matching or treats them specially.

### Cited Findings
- jdupes: "Normally linked files are treated as non-duplicates for safety." Use `-H` to consider them. Linked files can still land in one match set through a third, independently matching file (the "triangle problem"). — [jdupes README](https://codeberg.org/jbruchon/jdupes)
- fclones: "Files linked by symbolic links or hard links are not treated as duplicates." Stage 3 of its pipeline removes "duplicate files with the same inode id." — [fclones README](https://github.com/pkolaczk/fclones)
- dupeGuru has the option "Ignore duplicates hardlinking to the same file". When it is on, dupeGuru checks "if they refer to the same inode. If they do, they will not be considered duplicates. (Only for OS X and Linux)". — [dupeGuru preferences](https://dupeguru.voltaicideas.net/help/en/preferences.html)
- rmlint's defaults differ:
  - "Hardlinked files are treated as duplicates by default."
  - `--keep-hardlinked` never deletes files hardlinked to the original.
  - `-L`/`--no-hardlinked` considers only one file of a hardlinked set.
  - Its ranking criteria include `h/H` (hardlink count) and `o/O` (outside hardlink count). The default `pOma` prefers the file with the most hardlinks outside the scanned paths as the original.

  Source: [rmlint(1)](https://rmlint.readthedocs.io/en/latest/rmlint.1.html)
- Precize displays a file's link count, which is "1 unless it has at least one hard link". This is a search-result summary of eclecticlight content, not read in full. — [eclecticlight search result (tag: clone/Precize)](https://eclecticlight.co/tag/precize/page/2/)

### Inferences
- **Key on `(st_dev, st_ino)`** and collapse identical pairs into one candidate before hashing.
- **Treat outside links as zero-gain.** If `st_nlink > 1` and not all links are among the files to be deleted, deleting frees 0 bytes. Exclude such files from auto-select or report a 0-byte gain.
- **Choose a default.** Deleting a hard link is not dangerous, since data stays reachable through the other link, but it is pointless for space. The jdupes, fclones and dupeGuru default (exclude) suits a space-cleanup app.

### Gaps
- I did not find a Mac-specific source on whether Time Machine on APFS still uses directory hard links. That concerns older HFS+ backups; a scanner should exclude backup volumes anyway.

## 3. iCloud Drive "Optimize Mac Storage", dataless files and File Provider placeholders

### Takeaway
Since macOS Sonoma, evicted iCloud Drive files (and files from File Provider-based Dropbox, OneDrive, Google Drive and Box) are dataless. They keep their real name, full logical size and xattrs, but have no local data extents, and reading their contents triggers a download (materialization). A duplicate scanner must check `SF_DATALESS` in `st_flags` before opening any file. It must not hash dataless files, should not count them as reclaimable local space, and should turn off materialization for its own process.

### Cited Findings
- **Stubs were replaced in Sonoma.** Before Sonoma, an evicted file was a hidden `.Name.extn.icloud` stub under 200 bytes. In Sonoma there are "no stub files to be seen". Evicted items are dataless, with no data extents, and "they have the same file size as fully downloaded files." — [Oakley, macOS Sonoma has changed iCloud Drive radically](https://eclecticlight.co/2023/10/25/macos-sonoma-has-changed-icloud-drive-radically/)
- **Detection by `SF_DATALESS`.** Apple's guidance, as relayed by Oakley, is to "call `stat` or `getattrlist` and examine if `SF_DATALESS` is present in `stat.st_flags`." Cirrus shows evicted items as `NotDownloaded` using `ubiquitousItemDownloadingStatus` from `URLResourceValues`. — [Oakley, macOS Sonoma has changed iCloud Drive radically](https://eclecticlight.co/2023/10/25/macos-sonoma-has-changed-icloud-drive-radically/)
- **Flag value.** `#define SF_DATALESS 0x40000000 /* file is dataless object */` — local SDK header `MacOSX.sdk/usr/include/sys/stat.h`, line 359 (macOS 26 SDK). The same 0x40000000 value is used by a third-party Rust crate per a search snippet: [docs.rs lean-ctx cloud_files.rs](https://docs.rs/lean-ctx/3.7.5/src/lean_ctx/core/cloud_files.rs.html).
- **What a placeholder contains.** File Provider placeholders are "dataless files and folders consisting only of file attributes and extended attributes."
  - iCloud Drive adopted File Provider fully in Sonoma, and most major cloud services have since migrated.
  - Under "nonreplicated" providers (iCloud with Optimise Mac Storage on), files can be evicted to placeholders.
  - Providers may offer pinning to prevent eviction.

  Source: [Oakley, Explainer: File Provider and cloud services (May 2026)](https://eclecticlight.co/2026/05/09/explainer-file-provider-and-cloud-services/)
- **Third-party providers are dataless too.** Dataless files come from Apple's FileProvider framework, "which is what backs iCloud Drive and third-party cloud-storage providers like Dropbox, Google Drive, OneDrive, and Box."
  - "Reading the file (for example, opening it in an app) asks the provider to download the content again." Arq calls this materialization.
  - The daemons that do it are `bird` for iCloud Drive and `fileproviderd` for third-party providers.
  - With no user logged in, reads fail with "Operation timed out."
  - Arq offers three modes: treat as error, skip, or materialize and back up.

  Source: [Arq 7 docs: Backing up dataless files](https://arqbackup.com/documentation/arq7/English.lproj/datalessFiles.html) (vendor doc)
- **TN3150.** Apple's TN3150 "Getting ready for data-less files" advises apps to "avoid unnecessarily materializing dataless files" and to do content access "asynchronously off the main thread." It also says `stat` and `getattrlist` "both trigger the materialization of any intermediate folders in the file's path" if those folders are dataless. — quoted via [Michael Tsai, Getting Ready for Dataless Files](https://mjtsai.com/blog/2023/05/11/getting-ready-for-dataless-files); original [TN3150](https://developer.apple.com/documentation/technotes/tn3150-getting-ready-for-data-less-files). Apple's page body could not be fetched (it renders with JavaScript), so these quotes are second-hand through Tsai.
- **A commenter could not test the flag.** On Tsai's post, a developer reported that `stat()` on a non-local file returned "no such file". — [mjtsai comments](https://mjtsai.com/blog/2023/05/11/getting-ready-for-dataless-files) (single anecdote)
- **Process-level opt-out.** `setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, scope, policy)` accepts `IOPOL_MATERIALIZE_DATALESS_FILES_DEFAULT`, `_OFF` ("Disables materialization of dataless files by the current thread or process") and `_ON`. "New processes inherit the policy of their parent process." Header values: `IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES 3`, `_OFF 1`, `_ON 2`. — local `man setiopolicy_np` and `MacOSX.sdk/usr/include/sys/resource.h` lines 509–542.
- **Conflict on the default policy.** The local man page says the process-scope default follows "the system default policy (IOPOL_MATERIALIZE_DATALESS_FILES_OFF)". That conflicts with the practical behaviour Arq and Oakley describe, where ordinary reads materialize. A launchd plist key `MaterializeDatalessFiles` also exists (per a comment on [mjtsai](https://mjtsai.com/blog/2023/05/11/getting-ready-for-dataless-files)). The likely explanation is that GUI apps and processes launched through launchd/Foundation get materialization enabled. That is my inference and unverified, so set `_OFF` explicitly.
- **Materialization can time out.** A developer reported that the materialization policy produced persistent `ETIMEDOUT` errors on iCloud and Dropbox files. — [Apple Developer Forums thread 825820 (search snippet)](https://developer.apple.com/forums/thread/825820) (single source)
- **Detecting dataless state with `ls -lO`.** The `ls -O` option prints file flags (local `man ls`). I could not confirm that `ls -lO` prints a `dataless` keyword: the search found no source and `strings /bin/ls` shows no such keyword, because flag names come from libc `fflagstostr`. Oakley points to `stat`/`getattrlist` instead. — [eclecticlight search result](https://eclecticlight.co/2023/10/29/last-week-on-my-mac-dataless-files-arrive/)
- **Xattr sync on iCloud.** iCloud Drive syncs some xattrs and strips others. A 2026 Oakley piece reports that BSD flags such as `uappnd` and `schg` break iCloud Drive sync. — [Oakley, BSD flags are incompatible with iCloud Drive (Jun 2026)](https://eclecticlight.co/2026/06/02/bsd-flags-are-incompatible-with-icloud-drive/) (search snippet only); [Which extended attributes does iCloud preserve](https://eclecticlight.co/2020/03/18/which-extended-attributes-does-icloud-preserve-and-which-get-stripped/)

### Inferences
- **Check flags before reading.** Use `lstat` or `getattrlistbulk` with `ATTR_CMN_FLAGS` and test `SF_DATALESS` before opening any file. Skip dataless files from hashing entirely: hashing would download possibly gigabytes, and evicted files use roughly zero local space, so deleting them frees no local space but does delete the cloud copy.
- **Disable materialization at startup.** Call `setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF)` as a belt-and-braces guard. A read of a dataless file then errors instead of downloading; treat that error as "skip".
- **Deletion inside a cloud folder deletes everywhere.** Deleting a file in iCloud Drive or `~/Library/CloudStorage/*` deletes it from the cloud and all devices. Auto-select should avoid choosing files there, or should prefer keeping the cloud copy and deleting the local-only one.
- **Account for eviction.** Space "freed" by deleting a downloaded iCloud copy might also be recoverable non-destructively by eviction (`FileManager.evictUbiquitousItem`). This is a better default action for cloud-synced duplicates.
- **Watch intermediate folders.** Because `stat` can materialize dataless intermediate folders, enumerate with directory-relative calls (`getattrlistbulk` on an already-open directory fd) and check folder dataless state before descending.

### Gaps
- I could not fetch TN3150's body directly from Apple (JavaScript-rendered), so the exact errno returned when materialization is disabled is unconfirmed. Commonly cited values are `EDEADLK` or `ETIMEDOUT`; ETIMEDOUT appears only in the forum anecdote above.
- I did not confirm the exact `~/Library/CloudStorage/<Provider>-<account>` naming from an Apple or provider source in this session.
- I did not fetch Apple's `URLResourceKey.ubiquitousItemDownloadingStatus` page directly. Its use is cited through Oakley's Cirrus description.

## 4. Sparse files, extended attributes, resource forks, Finder tags and comments, quarantine: does "identical data fork" mean identical files?

### Takeaway
No. On macOS, metadata the user may value lives outside the data fork in xattrs: Finder tags, download URL and quarantine, the resource fork, FinderInfo (label colour), and provenance. Two files with identical data can differ in all of these. A data-fork hash defines "duplicate content", but the app should either compare or merge the metadata users can see, or tell users that tags and comments on deleted copies are lost. Sparse files have the same bytes but use less physical space.

### Cited Findings
- **Xattrs sit outside the data.** "Xattrs aren't stored with a file's data, nor with its normal attributes." They store "visible metadata such as Finder tags, and the URL used to download items from the internet."
  - `com.apple.quarantine` "can be attached to any file or folder without changing its data."
  - Copy intent preserves `com.apple.quarantine`, `com.apple.metadata:*` (with exceptions), `com.apple.ResourceFork` and `com.apple.FinderInfo`.
  - Copying to NFS strips all xattrs.

  Source: [Oakley, The secret life of the xattr (Apr 2026)](https://eclecticlight.co/2026/04/24/the-secret-life-of-the-xattr/)
- **FileProvider and resource forks.** For FileProvider, "the resource fork is content and isn't included in the extended attributes dictionary." — [Oakley, The secret life of the xattr](https://eclecticlight.co/2026/04/24/the-secret-life-of-the-xattr/)
- **Tool differences when copying.** Terminal `cp` copies all xattrs regardless of flags, while Finder copies honour xattr flags and can strip some. FileManager copies were observed to alter the quarantine value where `ditto` did not. — [Oakley, How do the Finder, Terminal and Spotlight handle extended attribute flags](https://eclecticlight.co/2019/07/26/how-do-the-finder-terminal-and-spotlight-handle-extended-attribute-flags); [Apple Developer Forums: Copy a file like ditto](https://developer.apple.com/forums/thread/676280) (search-result summaries)
- **Clones and xattrs.** A clone initially shares extended attributes with the original. — [Oakley, APFS: Files and clones](https://eclecticlight.co/2024/03/20/apfs-files-and-clones/)
- **Sparse files.** "For sparse files, it's the actual space used, so should be significantly less than the nominal data size." Oakley's sparse 100 GB VM used just over 50 GB. — [Oakley, When the Finder's size 'on disk' is misleading](https://eclecticlight.co/2026/09/05/when-the-finders-size-on-disk-is-misleading/). `EF_IS_SPARSE` flags sparse files. — [getattrlist(2)](https://keith.github.io/xcode-man-pages/getattrlist.2.html)
- **`EF_NO_XATTRS`** lets a scanner skip `listxattr()` for files without xattrs. — [getattrlist(2)](https://keith.github.io/xcode-man-pages/getattrlist.2.html)
- **Metadata-preserving dedupe.** rmlint `--dedupe` "makes no changes to dest's metadata." This is the opposite of deletion: the path and its metadata survive. — [rmlint(1)](https://rmlint.readthedocs.io/en/latest/rmlint.1.html)

### Inferences
- **Hash the data fork** for "same content". Compare `com.apple.ResourceFork` too, or exclude files that have one, so that two files with the same data fork but different resource forks (legacy fonts, old Mac docs) are not deleted.
- **Keep the copy with more user metadata.** Before auto-selecting, prefer to keep the copy with user-visible metadata (Finder tags `com.apple.metadata:_kMDItemUserTags`, Finder comments, `kMDItemWhereFroms`). Optionally merge tags from deleted copies into the keeper. Do not copy `com.apple.quarantine` onto the keeper; the keeper's own quarantine state should stand.
- **Quarantine as a signal.** A downloaded copy in ~/Downloads with quarantine and WhereFroms is commonly the redundant one. That is a reasonable "prefer to delete" signal, consistent with Gemini 2 auto-selecting Downloads (section 5).
- **Report sparse savings correctly.** For sparse copies, report allocated size or PRIVATESIZE, not logical size.

### Gaps
- No primary source found for whether Finder comments (stored in `.DS_Store` and/or the `com.apple.metadata:kMDItemFinderComment` xattr) survive Finder duplication. This is unverified.
- None of the duplicate-finder docs reviewed (dupeGuru, jdupes, fclones, rmlint, Czkawka) state that they compare xattrs or resource forks. All appear to compare the data fork only, but that is inferred from absence.

## 5. Which copy to keep: defaults of established tools

### Takeaway
Most tools keep the "first" file under an ordering the user controls: command-line path order, a reference or protected folder, or oldest mtime. GUI tools add location heuristics. Gemini 2 auto-selects copies in Downloads and Desktop but not Documents, and never auto-selects "similar" files. Protected reference paths and "never select the last copy" are near-universal safeguards. No tool documentation found states a rule about Photos libraries or app bundles. That rule is advisable but has to come from your own design.

### Cited Findings
- **Gemini 2 (MacPaw):** (vendor docs)
  - It "automatically selects identical files ('duplicates'), which can be safely removed with a single click, without review." Selected files go to the Trash.
  - When scanning Home, duplicates in Downloads and Desktop are auto-selected; those in Documents are not.
  - Similar files are never auto-selected.
  - It learns from user choices and proposes new rules (for example "select newer files"). Rules are editable in Preferences > Smart Selection.

  Source: [MacPaw: All about Smart Cleanup](https://macpaw.com/support/gemini/knowledgebase/gemini-2-smart-selection). The user guide says Smart Select aims to leave at least one copy: [Gemini 2 User Guide PDF](https://cdn2.macpaw.com/manuals/Gemin-2-User-Guide-29062021.pdf) (search-snippet summary). A "last copy" warning is reported by a review: [iDownloadBlog review](https://www.idownloadblog.com/?p=417874) (search snippet). A third-party claim that it keeps the "original location or date" copy is unverified.
- **dupeGuru:**
  - "A file from a reference folder will always be reference in a duplicate group."
  - Otherwise "dupeGuru assumes that you always want to keep the biggest file."
  - The reference file can never be marked.
  - Delete options are: send to Trash (default), replace with a hardlink or symlink, or "Directly delete" (meant for troubleshooting).
  - It recommends always reviewing before deleting and documents no re-verification before deletion.

  Source: [dupeGuru: Results](https://dupeguru.voltaicideas.net/help/en/results.html)
- **rmlint:** The default rank is `-S pOma`: first-named path, then most outside hardlinks, then oldest mtime, then alphabetical basename. Tagged paths (after `//`) take priority. `--keep-all-tagged` never deletes in tagged paths, and `--must-match-tagged` only reports groups with a tagged member. — [rmlint(1)](https://rmlint.readthedocs.io/en/latest/rmlint.1.html)
- **jdupes:** It sorts by filename by default (`-o name`) or mtime (`-o time`). `-O` uses command-line parameter order. With `-N --delete` it "preserve[s] the first file in each set." — [jdupes README](https://codeberg.org/jbruchon/jdupes)
- **fclones:** "It keeps the files given at the beginning of each list, and removes / replaces the files given at the end of each list." It has `--priority newest|oldest` and more options. `--dry-run` prints commands without running them. — [fclones README](https://github.com/pkolaczk/fclones)
- **Czkawka:** Its delete methods are `AllExceptNewest`, `AllExceptOldest`, `OneNewest`, `OneOldest`, `AllExceptBiggest`, `AllExceptSmallest`, `OneBiggest`, `OneSmallest` and `HardLink`, plus `set_dry_run` and `set_move_to_trash`. Reference paths are "protected: they cannot be modified, moved, or deleted by any automatic action in the UI or CLI." — [Czkawka Instruction_Core](https://github.com/qarmin/czkawka/blob/master/instructions/Instruction_Core.md); [Czkawka Instruction](https://github.com/qarmin/czkawka/blob/master/instructions/Instruction.md)
- **PhotoSweeper:** It has user-ordered Auto Mark rules in Preferences (size, dimensions, DPI, rating, keywords, "Quality Score"). It works with Apple Photos, Lightroom and Capture One.
  - Removal is reversible: files go to the Trash, are marked "Rejected" in Lightroom, or are moved to an album in Photos.
  - Its "Duplicate Files" mode uses checksums for 100% identical files.
  - One user reported that the defaults could keep `name (1)` over the original shorter name.

  Sources: App Store listing and review snippets only, which are weak sources: [App Store](https://apps.apple.com/app/id463362050), [Nektony review](https://nektony.com/reviews/best-duplicate-photo-finder), [9to5Mac](https://9to5mac.com/2015/05/07/how-to-shrink-mac-photo-library-delete-duplicates/).

### Inferences
- **A conservative default keeper order for a Mac cleanup app**, each rule backed by the tool behaviour above:
  1. Never delete from protected or reference locations (dupeGuru, Czkawka, rmlint tags).
  2. Prefer deleting copies in Downloads, Desktop or Trash-adjacent locations (Gemini 2).
  3. Prefer keeping the copy outside cloud-sync roots, or the one that is materialized. See section 3.
  4. Prefer keeping the copy with Finder tags or comments.
  5. Prefer keeping the oldest mtime or birthtime (rmlint default `m`, Czkawka `AllExceptOldest`).
  6. Prefer keeping the shorter name or the name without " copy" / " (1)" suffixes. This addresses the PhotoSweeper complaint.
  7. Break ties by shortest path or depth (rmlint `d`).
- **Never auto-select files inside packages or libraries.** Exclude `*.photoslibrary`, `*.musiclibrary`, Lightroom catalogs, app bundles (`*.app`), `*.framework` and other packages (`NSURLIsPackageKey`). Deleting a member file corrupts the container, and Photos expects library changes through PhotoKit. No source above states this rule explicitly; it follows from PhotoSweeper deliberately removing items via Photos albums or rejection rather than the file system.
- **Never auto-select every member of a group.** Require at least one keeper that still exists and was re-verified at deletion time.
- **Prefer Trash or reversible deletion,** consistent with Gemini 2, dupeGuru and PhotoSweeper defaults.

### Gaps
- MacPaw does not publicly document Smart Select's full rule list or tie-breaker, so it is a vendor black box.
- No tool documentation found explicitly states "never auto-select inside the Photos library or app bundles". This is a design recommendation, not a cited practice.
- dupeGuru's Re-Prioritize dialog criteria page was not fetched.

## 6. Hashing strategy and safety (TOCTOU, budgets)

### Takeaway
The standard pipeline is: group by size, drop inode-identical entries, hash a small prefix (and/or suffix), then hash the whole file, then optionally compare byte-for-byte. Safety-focused tools add file-change checks between scan and action and warn loudly about modes that skip the final verification. Re-verify size, mtime/ctime, inode and dataless state immediately before each delete.

### Cited Findings
- **jdupes.**
  - Its order: size, then a hash of "the first 4 KiB", then a full-file hash, then "actual file data is compared to verify that they are duplicates."
  - `-Q` skips the byte compare. Its warnings: "This may delete non-duplicates!", "There is a small but significant risk of a hash collision," and "Do not use it on ANY data set for which any amount of data loss is unacceptable."
  - `-t` "disables file change checks during/after scanning" and "opens a security vulnerability that is called a TOCTTOU (time of check to time of use) vulnerability". By implication, change checks are on by default.

  Source: [jdupes README](https://codeberg.org/jbruchon/jdupes)
- **fclones.** Its order: size grouping, remove same-inode files, hash a small block at the start, then at the end, then the full contents. The default hash is `metro` (128-bit, non-cryptographic). It has `--dry-run`. Its README does not document re-verification before removal. — [fclones README](https://github.com/pkolaczk/fclones)
- **Czkawka.** It groups by size, then by a "prehash of the first and last 4 KB", then by full hash. Hash options are Blake3 (cryptographic), XXH3 (fastest) and CRC32. A prehash cache "re-scans only need to fully hash new or changed files". — [Czkawka Instruction_Core](https://github.com/qarmin/czkawka/blob/master/instructions/Instruction_Core.md); [Czkawka Instruction](https://github.com/qarmin/czkawka/blob/master/instructions/Instruction.md)
- **rmlint.**
  - The default digest is blake2b (512-bit), and `-p` (paranoid) does a byte-by-byte comparison.
  - "rmlint itself WILL NOT DELETE ANY FILES"; it emits a script.
  - `-p` gives "a full re-check of the duplicate against the original before it deletes the file".
  - `--replay` "will ignore files whose mtime changed in the meantime".
  - The `--xattr` checksum cache is invalidated by mtime.

  Source: [rmlint(1)](https://rmlint.readthedocs.io/en/latest/rmlint.1.html)
- **dupeGuru** documents no pre-deletion re-verification. — [dupeGuru: Results](https://dupeguru.voltaicideas.net/help/en/results.html)

### Inferences
- **Recommended pipeline:**
  1. Enumerate with `getattrlistbulk`, collecting size, inode, `st_nlink`, flags (`SF_DATALESS`), `EXT_FLAGS`, `CLONEID` and PRIVATESIZE.
  2. Skip dataless, package-member and zero-length files.
  3. Group by size and collapse identical inodes.
  4. Collapse identical CLONEIDs, which are known identical at zero gain.
  5. Hash prefix and suffix (4–16 KB), then hash the full file with a cryptographic or 128-bit+ hash (BLAKE3).
  6. Optionally compare byte-for-byte before deleting large or high-value files.
- **Before each deletion,** re-`lstat` both the victim and the keeper. Confirm unchanged dev/ino/size/mtime/ctime, that neither is dataless, and that the keeper still exists. Ideally re-hash or byte-compare: this is what rmlint `-p` and jdupes change checks do. Operate on an fd opened with `O_NOFOLLOW` and delete relative to the parent directory fd, to avoid symlink races.
- **Budgets.** Cap total bytes hashed per run and skip files under a minimum size. Czkawka examples use 8 KB to 256 KB thresholds, which are illustrative rather than defaults. Cache hashes keyed by (dev, ino, size, mtime), invalidated on change as rmlint and Czkawka do, so re-scans are cheap.

### Gaps
- I found no authoritative benchmark comparing partial-hash sizes on APFS SSDs.
- Whether the fclones and Czkawka delete paths re-verify files before deletion is not documented in the pages fetched.
