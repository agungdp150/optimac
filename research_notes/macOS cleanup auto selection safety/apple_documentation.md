# What Apple's own documentation says about disposable user-domain files on macOS

Research date: 2026-11-13. Sources: developer.apple.com (current DocC pages, read through their JSON endpoints so quotes are exact), the developer.apple.com Documentation Archive (legacy, dated below), support.apple.com articles and the Mac User Guide for macOS Tahoe 26, and the man pages that ship with macOS (Darwin 25.6). Text in quotation marks is verbatim.

**Source freshness:**
- **Current:** developer.apple.com/documentation pages, the support.apple.com articles (published July to October 2026), and the Mac User Guide pages under `/26/mac/26` (macOS Tahoe 26).
- **Legacy:** The *File System Programming Guide* is an archived document whose footer reads "Updated: 2018-04-09". Treat it as Apple's long-standing convention, not as a current guarantee.
- **Local man pages:** These were read on this machine, with `man confstr`, `man tmutil`, `man 8 deleted` and `xcrun simctl help delete`. They have no web URL, so they are cited by command.

---

## Q1. What do Apple's developer docs say about the semantics of ~/Library/Caches, Application Support, Logs, Saved Application State, Containers / Group Containers, and /private/var/folders (NSTemporaryDirectory, DARWIN_USER_CACHE_DIR)?

### Takeaway
Apple defines `Caches` and `tmp` as regenerable, "purgeable" data that apps must not rely on. It defines `Application Support`, `Containers` and `Group Containers` as places for real app data, sometimes user data. On macOS, Apple says the system never removes a Group Container. Two `confstr` directories under /var/folders behave differently:
- `DARWIN_USER_TEMP_DIR` (`…/T/`): the system may clean files there after 3 days without access.
- `DARWIN_USER_CACHE_DIR` (`…/C/`): the system does not clean it automatically, but a safe boot removes it.

### Cited Findings

**~/Library/Caches**
- The current API doc for `FileManager.SearchPathDirectory.cachesDirectory` gives its abstract as: "Discardable cache files (`Library/Caches`)." The constant has been available since macOS 10.0. — [Apple Developer: cachesDirectory](https://developer.apple.com/documentation/foundation/filemanager/searchpathdirectory/cachesdirectory)
- The current doc *Optimizing Your App's Data for iCloud Backup* defines purgeable data as follows: "Purgeable data is app data that the system can delete without affecting the app's execution and that the app can re-create if it needs." — [Apple Developer: Optimizing Your App's Data for iCloud Backup](https://developer.apple.com/documentation/foundation/optimizing-your-app-s-data-for-icloud-backup)
- The same doc says: "The app container provides two directories for storing purgeable data: `/tmp` [and] `/Library/Caches`. The system periodically purges these directories, so iCloud Backup excludes them by default." It also says "Don't use these directories to exclude nonpurgeable data from iCloud Backup." This doc is written for the app container and iCloud Backup, which is iOS-centric. — [same](https://developer.apple.com/documentation/foundation/optimizing-your-app-s-data-for-icloud-backup)
- **LEGACY, 2018-04-09**, *macOS Library Directory Details*, Caches row: "Contains cached data that can be regenerated as needed. Apps should never rely on the existence of cache files. Cache files should be placed in a directory whose name matches the bundle identifier of the app." — [File System Programming Guide: macOS Library Directory Details (archive)](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/MacOSXDirectories/MacOSXDirectories.html)
- **LEGACY, 2018-04-09**, *File System Basics*, Table 1-3: "Use this directory to write any app-specific support files that your app can re-create easily. Your app is generally responsible for managing the contents of this directory and for adding and deleting files as needed." — [File System Programming Guide: File System Basics (archive)](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html)
- **LEGACY, iOS-specific:** "In iOS 5.0 and later, the system may delete the `Caches` directory on rare occasions when the system is very low on disk space. This will never occur while an app is running." The iOS placement guidance adds: "Note that the system may delete the `Caches/` directory to free up disk space, so your app must be able to re-create or download these files as needed." — [same](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html)

**~/Library/Application Support**
- The current API doc for `applicationSupportDirectory` gives its abstract as: "Application support files (`Library/Application Support`)." — [Apple Developer: applicationSupportDirectory](https://developer.apple.com/documentation/foundation/filemanager/searchpathdirectory/applicationsupportdirectory)
- **LEGACY, 2018:** "Contains all app-specific data and support files. These are the files that your app creates and manages on behalf of the user and can include files that contain user data." — [macOS Library Directory Details (archive)](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/MacOSXDirectories/MacOSXDirectories.html)
- **LEGACY, 2018:** "Use this directory to store all app data files except those associated with the user's documents. For example, you might use this directory to store app-created data files, configuration files, templates… A game might use this directory to store new levels purchased by the user and downloaded from a server." — [File System Basics (archive)](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html)
- Apple itself stores iOS device backups in `~/Library/Application Support/MobileSync/Backup/` (see Q4). This is a concrete example of irreplaceable user data inside Application Support. — [Apple Support 108809](https://support.apple.com/en-us/108809)

**~/Library/Logs**
- **LEGACY, 2018:** "Contains log files for the console and specific system services. Users can also view these logs using the Console app." — [macOS Library Directory Details (archive)](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/MacOSXDirectories/MacOSXDirectories.html)
- The current Mac User Guide for Tahoe 26 says macOS itself "clears caches and logs that are safe to delete" when space is needed (see Q2). — [Mac User Guide (macOS 26): Optimize storage space](https://support.apple.com/en-us/guide/mac-help/sysp4ee93ca4/26/mac/26)

**~/Library/Saved Application State**
- The archived *macOS Library Directory Details* page does not mention it. I found no Apple primary source that describes deleting it. See Gaps.

**~/Library/Containers (App Sandbox)**
- **Current doc:** "When your sandboxed app launches for the first time, macOS creates a sandbox container on the file system (in `~/Library/Containers`) and associates it with your app. Your app has full read and write access to its sandbox container…" — [Apple Developer: Accessing files from the macOS App Sandbox](https://developer.apple.com/documentation/security/accessing-files-from-the-macos-app-sandbox)
- **Current doc:** "In macOS 14 and later, the operating system uses your app's code signature to associate it with its sandbox container. If your app tries to access the sandbox container owned by another app, the system asks the person using your app whether to grant access." This also applies to an app that lacks the sandbox entitlement and tries to access another app's container. — [same](https://developer.apple.com/documentation/security/accessing-files-from-the-macos-app-sandbox)
- **LEGACY, 2018:** "macOS apps that are sandboxed have all their `Application Support`, `Cache`, temporary directories and other related documents stored within a directory located at a system-defined path…" — [File System Basics (archive)](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html)
- **LEGACY, 2018:** Containers "Contains the home directories for any sandboxed apps. (Available in the user domain only.)" — [macOS Library Directory Details (archive)](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/MacOSXDirectories/MacOSXDirectories.html)

**~/Library/Group Containers**
- **Current doc:** "For a sandboxed app in macOS, the group directory is located at `~/Library/Group Containers/<application-group-id>`… **The system creates this directory automatically the first time your app needs it and never removes it.**" — [Apple Developer: containerURL(forSecurityApplicationGroupIdentifier:)](https://developer.apple.com/documentation/foundation/filemanager/containerurl(forsecurityapplicationgroupidentifier:))
- The same doc contrasts this with iOS: "Unlike in macOS, when all the apps in a given app group are removed from the device, the system detects this condition and removes the corresponding group directory as well." — [same](https://developer.apple.com/documentation/foundation/filemanager/containerurl(forsecurityapplicationgroupidentifier:))
- The same doc describes the layout: "The system also creates the `Library/Application Support`, `Library/Caches`, and `Library/Preferences` subdirectories inside the group directory the first time you use it." — [same](https://developer.apple.com/documentation/foundation/filemanager/containerurl(forsecurityapplicationgroupidentifier:))

**~/Library/Preferences** (for context)
- **LEGACY, 2018:** "Contains the user's preferences. You should never create files in this directory yourself. To get or set preference values, you should always use the `NSUserDefaults` class…" — [macOS Library Directory Details (archive)](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/MacOSXDirectories/MacOSXDirectories.html)

**/private/var/folders (per-user temp and cache dirs)**
- `NSTemporaryDirectory()` "Returns the path of the temporary directory for the current user". The doc points to `FileManager.url(for:in:appropriateFor:create:)` as "the preferred means of finding the correct temporary directory." It says nothing about lifetime. — [Apple Developer: NSTemporaryDirectory()](https://developer.apple.com/documentation/foundation/nstemporarydirectory())
- **Local man page** `man 3 confstr` (macOS 26, Darwin 25.6), `_CS_DARWIN_USER_TEMP_DIR`: "Provides the path to a user's temporary items directory… is a good location for temporary files. **By default, files in this location may be cleaned (removed) by the system if they are not accessed in 3 days.**" — local man page `confstr(3)`, no web URL.
- **Local man page** `man 3 confstr`, `_CS_DARWIN_USER_CACHE_DIR`: "Provides the path to the user's cache directory… is a good location for user cache data **as it will not be automatically cleaned by the system. Files in this location will be removed during safe boot.**" — local man page `confstr(3)`, no web URL.
- **Local man page** `man 3 confstr`, `_CS_DARWIN_USER_DIR`: "is not intended to be used for sensitive or temporary file storage, as all users can see files created here." — local man page `confstr(3)`, no web URL.
- **LEGACY, 2018, iOS:** "the system may purge this directory when your app is not running." The macOS section lists `/tmp` only as "Contains temporary files created by apps and the system." — [File System Basics (archive)](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html)

### Inferences
- **(Inferred)** Apple's contract makes `~/Library/Caches/<bundle-id>` the clearest candidate for preselection. Apple calls it "discardable" and says apps "should never rely on the existence of cache files". One caveat: the strong guarantees, that the system deletes Caches and apps must tolerate it, are written for iOS. On macOS, Apple's 2018 wording is only "apps should never rely on", so a badly behaved third-party app might still keep non-regenerable data there.
- **(Inferred)** `Application Support`, `Containers` and `Group Containers` should not be preselected wholesale. Apple describes them as app data that "can include files that contain user data". Apple says it never removes Group Containers on macOS, so orphaned Group Containers will accumulate. Removing them is a judgment call the system deliberately does not make.
- **(Inferred)** Since macOS 14, touching another app's `~/Library/Containers/<id>` triggers a TCC-style consent prompt or a denial. A cleanup tool that walks Containers should expect access errors and prompts. Container contents should be treated as app-owned.
- **(Inferred)** `$TMPDIR` (`/var/folders/…/T`) is already reaped by the system after 3 days of no access. `/var/folders/…/C` is not reaped automatically but is wiped by safe boot, so Apple treats it as disposable at a system level. Deleting from either one while the owning process runs could break that process. Apple's iOS docs stress "never while an app is running".

### Gaps
- I found no current, non-archived Apple developer page with a macOS-specific sentence that the system purges `~/Library/Caches` under low space. The direct statements are iOS (archive) or app-container/iCloud-Backup oriented. The macOS-level statement comes from the Mac User Guide (Q2) and refers only to "caches and logs that are safe to delete".
- I found no Apple primary-source text on `~/Library/Saved Application State` (window restoration state) or on whether deleting it is safe. The `NSWindowRestoration` API doc does not describe the on-disk location.
- I found no Apple doc on what happens to `~/Library/Containers/<id>` when a user deletes an app on macOS.

---

## Q2. What do support.apple.com and Mac User Guide articles say about Storage settings, "System Data"/"Other", purgeable space, and what macOS itself offers to remove?

### Takeaway
Apple's own Storage settings in Tahoe 26 offer:
- Store in iCloud.
- Optimize Storage (watched Apple TV media, old Mail attachments).
- Empty Trash automatically after 30 days.
- Per-category browsing and deletion (Applications, Documents, iOS Files, etc.).

Apple states that macOS itself clears "caches and logs that are safe to delete" when space is needed. It says users "can't manage" System Data, because "The contents are managed by macOS".

### Cited Findings
- **Where Storage lives:** "In macOS Ventura 13 or later, choose System Settings from the Apple menu, click General in the sidebar, then click Storage on the right." — [Apple Support 102624: Free up storage space on Mac](https://support.apple.com/en-us/102624) (Published Date: October 16, 2026)
- **System Data definition (support article):** "The category named System Data (or Other) is a general category that measures the storage space used by all Apple and third-party files that don't belong to any more specific category." — [Apple Support 102624](https://support.apple.com/en-us/102624)
- **System Data definition (Mac User Guide, Tahoe 26):** "System Data: Contains files that don't fall into the categories listed here. This category primarily includes files and data used by the system, such as log files, caches, VM files, and other runtime system resources. Also included are temporary files, fonts, app support files, and plug-ins. **You can't manage the contents of this category. The contents are managed by macOS, and the category varies in size depending on the current state of your Mac.** You can manage your data that falls outside the other categories using the Finder or the third-party apps that created it." — [Mac User Guide (macOS 26): Find and delete files on your Mac](https://support.apple.com/guide/mac-help/find-and-delete-files-on-your-mac-syspf5a64aa6/26/mac/26)
- **Free vs. available vs. purgeable:** "Free storage space is physically open space on a disk. Available storage space includes free storage space and purgeable disk caches (disk space might be both available and used at the same time)." — [same](https://support.apple.com/guide/mac-help/find-and-delete-files-on-your-mac-syspf5a64aa6/26/mac/26)
- **What macOS clears automatically (key quote):** "When space is needed on your Mac, macOS also clears caches and logs that are safe to delete, including temporary database files, interrupted downloads, staged macOS and app updates, Safari website data, and more." — [Mac User Guide (macOS 26): Optimize storage space on your Mac](https://support.apple.com/en-us/guide/mac-help/sysp4ee93ca4/26/mac/26)
- **The three recommendations in Tahoe 26:**
  - "Store in iCloud": "…optimize storage by keeping only recently opened files on your Mac when space is needed."
  - "Optimize Storage": "Save space by automatically removing Apple TV movies and TV shows that you already watched, and by keeping only recent email attachments on this Mac when storage space is needed."
  - "Empty Trash automatically": "Automatically erase items that have been in the Trash for more than 30 days."
  - Also: "If your disk is partitioned, recommendations apply to only the partition that contains your home directory."
  - — [same](https://support.apple.com/en-us/guide/mac-help/sysp4ee93ca4/26/mac/26)
- **Categories Apple exposes for manual deletion (Tahoe 26):**
  - "Applications: Contains all the apps installed on your Mac (excluding system applications)."
  - "Documents: … You can quickly view large files or downloads…"
  - "iOS files: Contains iOS backups and firmware."
  - "TV, Music, Books, and Podcasts: Each app category contains purchases from that specific app, which can be removed from your Mac and downloaded again."
  - "Trash", "Other Users", "macOS: Contains macOS system applications and files."
  - "Note: The categories you see vary, depending on the apps and files on your Mac." To delete: "select it, then click Delete; click Show in Finder to view the file in the Finder."
  - — [Mac User Guide (macOS 26): Find and delete files](https://support.apple.com/guide/mac-help/find-and-delete-files-on-your-mac-syspf5a64aa6/26/mac/26)
- **Manual steps Apple recommends to users:**
  - "Delete music, movies, podcasts, or other media, especially if it's media that you can stream or download again as needed."
  - "Delete files in your Downloads folder."
  - "choose Mailbox > Erase Junk Mail… Also choose Mailbox > Erase Deleted Items."
  - "If you use your Mac to back up your iPhone or iPad, you can delete old backups."
  - "Delete or uninstall apps that you no longer use."
  - "Check the Users folder on Macintosh HD for any undeleted data belonging to deleted users"
  - "Empty your Trash. When you move a file to the Trash, its storage space doesn't become available until you empty the Trash."
  - — [Apple Support 102624](https://support.apple.com/en-us/102624)
- **Safe mode cache clearing:** "To temporarily make more storage space available, it might help to start up your Mac in safe mode. Your Mac then clears certain system caches, which are automatically created again as needed." — [Apple Support 102624](https://support.apple.com/en-us/102624)

### Inferences
- **(Inferred)** Apple's own model is that Downloads, old iOS backups, unused apps, media you can re-download, and Trash are things the user reviews and deletes. Apple offers browsing, not automatic deletion, for all of these except Trash after 30 days, which is opt-in. A preselecting cleanup tool would be more aggressive than Apple's own UI for these categories.
- **(Inferred)** Apple names the things macOS removes itself: "temporary database files, interrupted downloads, staged macOS and app updates, Safari website data". These are the closest Apple-endorsed "safe to delete" list. Deleting them proactively mostly duplicates what macOS does on demand.
- **(Inferred)** "System Data … You can't manage the contents of this category" is Apple's position that users, and by extension third-party tools, should not try to manage it directly.

### Gaps
- The Tahoe 26 Mac User Guide category list has no "Developer" or "Xcode" category, and no "iOS Software Updates" item separate from "iOS files … backups and firmware". Older macOS versions showed a Developer category. I did not find a current Apple page that documents it.
- No current Apple page says that Downloads is reviewed automatically. "Documents" offers a "Downloads" button for browsing.
- When I fetched the unversioned Mac User Guide URL, a summariser reported a version selector listing "macOS 27". That is unverified and likely a summariser error, so these notes rely only on the explicit `/26/mac/26` pages.

---

## Q3. What does Apple say about APFS local Time Machine snapshots and purgeable space? Should a third-party tool delete them?

### Takeaway
Apple says local snapshots count as available space and that "You don't need to think about how much storage space local snapshots are using". Time Machine deletes them automatically as they age (24 hours) or as space is needed. Apple's only documented manual-deletion method is to temporarily switch Time Machine to manual backups. Apple does not recommend that users or third-party tools delete them. `tmutil deletelocalsnapshots` exists but is a low-level CLI.

### Cited Findings
- "Time Machine saves one snapshot of your startup disk approximately every hour, and keeps it for 24 hours. It keeps an additional snapshot of your last successful Time Machine backup until space is needed. And in macOS High Sierra 10.13 or later, another snapshot is saved before installing any macOS update." — [Apple Support 102154: About Time Machine local snapshots](https://support.apple.com/en-us/102154) (Published Date: July 06, 2026)
- "**Your Mac counts the space used by snapshots as available storage. You don't need to think about how much storage space local snapshots are using, because they don't use the space needed for tasks like downloading files, copying files, or installing new software.** Even so, Time Machine stores snapshots only on disks that have plenty of free space." — [Apple Support 102154](https://support.apple.com/en-us/102154)
- "Time Machine automatically deletes snapshots as they age or as space is needed for other things, but if you want to delete local snapshots manually, turn off automatic backups temporarily". For Ventura 13 or later: "click Options, choose Manually from the Backup Frequency pop-up menu, then click Done… Wait a few minutes to allow local snapshots to be deleted. Then turn on automatic backups again." — [Apple Support 102154](https://support.apple.com/en-us/102154)
- Mac User Guide (Tahoe 26): "These local snapshots are stored on the same disk as the original files… These files are stored for up to 24 hours or until space is needed on the disk. When your normal backup disk isn't available, local snapshots are a convenient way to retrieve earlier versions of your files." Also: "Important: Local snapshots are only stored on disks using Apple File System (APFS) when Back Up Automatically is selected" and "Time Machine automatically removes local snapshots when space is needed on the disk or when they are older than 24 hours." — [Mac User Guide (macOS 26): About Time Machine local snapshots](https://support.apple.com/en-us/guide/mac-help/mh35933/26/mac/26)
- **Local man page** `man tmutil`:
  - `deletelocalsnapshots {mount_point | date}`: "delete all local Time Machine snapshots on all mounted disks for the specified date… If a disk is specified, delete all local Time Machine snapshots on the specified disk".
  - `thinlocalsnapshots mount_point [purge_amount] [urgency]`: "tmutil will attempt (with urgency level 1-4) to reclaim purge_amount in bytes by thinning snapshots."
  - "Several, but not all, verbs require root and Full Disk Access privileges."
  - — local man page `tmutil(8)`, no web URL.

### Inferences
- **(Inferred)** Apple's documentation argues against preselecting local snapshots. Apple counts them as available space, purges them automatically, and they are the user's only restore point when the backup disk is absent, including the pre-update snapshot. Deleting them yields no "available" space in Apple's own accounting, so the gain appears only in "free" space.
- **(Inferred)** If a tool offers this at all, `tmutil thinlocalsnapshots` (asking the system to thin) fits Apple's model better than `deletelocalsnapshots`.

### Gaps
- No Apple page explicitly says "third-party apps should not delete snapshots". The recommendation above is inferred from Apple's statements that management is unnecessary and automatic.
- Apple's current support pages do not use the word "purgeable" for snapshots. The Mac User Guide's "purgeable disk caches" wording does not explicitly include snapshots. Third-party commentary links the two, and I did not use it.

---

## Q4. What does Apple say about removing iOS/iPadOS device backups, Xcode DerivedData, Archives, iOS DeviceSupport, and simulators?

### Takeaway
Apple documents iOS backups at `~/Library/Application Support/MobileSync/Backup/` and warns: "After you delete a backup, you can't recover it." Apple's sanctioned path is Finder or Apple Devices > Manage Backups, and it offers "Archive" to protect a backup. For Xcode, Apple documents removing simulator runtimes and platforms in Xcode > Settings > Components, and `simctl delete unavailable`. Apple says you "must retain the Xcode archive for each build of your app you distribute". I found no Apple page about DerivedData or iOS DeviceSupport.

### Cited Findings
- **iOS backup location:** "From the Go menu in the Finder menu bar, choose Go to Folder. Paste or type ~/Library/Application Support/MobileSync/Backup/ …" — [Apple Support 108809: Locate and manage backups of your iPhone, iPad, and iPod touch](https://support.apple.com/en-us/108809) (Published Date: October 20, 2026)
- **Warning and sanctioned UI:** "**After you delete a backup, you can't recover it. If you want to keep a backup, archive it so that later backups don't replace it.**" The steps: Open Finder or Apple Devices app → connect device → "Click the General tab, then click Manage Backups. Right-click the backup that you want. Choose Delete, Archive, or Show In Finder." — [Apple Support 108809](https://support.apple.com/en-us/108809)
- Apple's free-space article lists old iPhone and iPad backups as deletable: "If you use your Mac to back up your iPhone or iPad, you can delete old backups." — [Apple Support 102624](https://support.apple.com/en-us/102624)
- The Storage category "iOS files: Contains iOS backups and firmware." — [Mac User Guide (macOS 26): Find and delete files](https://support.apple.com/guide/mac-help/find-and-delete-files-on-your-mac-syspf5a64aa6/26/mac/26)
- **Xcode components and simulator runtimes (current doc):** "Xcode lets you manage optional components yourself so that you can install only the components you use and remove the ones you don't." Also: "To manage your components, choose Xcode > Settings and click Components in the sidebar. Xcode shows the installed and enabled components along with the amount of storage you can recover if you remove them. To remove or disable components you no longer use and recover their storage space, click the information button next to the component. In the dialog that appears, click Delete or Turn Off depending on the component." The pane is "Components" in current Xcode; older Xcode called it "Platforms". — [Apple Developer: Downloading and installing additional Xcode components](https://developer.apple.com/documentation/xcode/downloading-and-installing-additional-xcode-components)
- **Simulators (local tool help)** `xcrun simctl help delete`: "Delete specified devices, unavailable devices, or all devices. Usage: simctl delete <device> [... <device n>] | unavailable | all. Specifying unavailable will delete devices that are not supported by the current Xcode SDK." — local `simctl` help output, no web URL.
- **Archives:** "When archiving your app for distribution, Xcode gathers all binaries and `dSYM` files for your app and stores them inside the Xcode archive." Also: "**You must retain the Xcode archive for each build of your app you distribute. Without this archive, you might not be able to diagnose an issue from crash reports.**" — [Apple Developer: Building your app to include debugging information](https://developer.apple.com/documentation/xcode/building-your-app-to-include-debugging-information)

### Inferences
- **(Inferred)** iOS backups should not be preselected. Apple warns that deletion is unrecoverable, provides an "Archive" mechanism, and routes deletion through a per-device UI. A tool should at minimum exclude archived backups and show device name and date. Telling archived from latest backups requires reading backup metadata, which I did not research.
- **(Inferred)** `xcrun simctl delete unavailable` is Apple's sanctioned, narrowly scoped deletion of simulator devices the current SDK no longer supports. It is a better candidate for preselection than deleting `~/Library/Developer/CoreSimulator` by hand.
- **(Inferred)** `~/Library/Developer/Xcode/Archives` should never be preselected. Apple says developers "must retain" archives of distributed builds.
- **(Inferred, not Apple-sourced)** DerivedData is widely treated as a regenerable build cache. Apple's current docs I found do not state this, so it rests on community convention only.

### Gaps
- I found no developer.apple.com or support.apple.com page that documents `~/Library/Developer/Xcode/DerivedData` or says it is safe to delete. A search returned only third-party blogs.
- I found no Apple doc on `~/Library/Developer/Xcode/iOS DeviceSupport` (device symbol caches).
- I found no current Apple doc on "iOS software updates" (.ipsw firmware files) as a separate removable item, beyond "iOS files: Contains iOS backups and firmware."

---

## Q5. Does Apple say anything about deleting .dmg/.pkg installers after installing apps, or about DiagnosticReports / crash logs?

### Takeaway
Apple's general advice is "Delete files in your Downloads folder". It does not specifically address deleting .dmg/.pkg installers in the pages I found. For crash logs, Apple documents finding them through Console > Crash Reports and sharing them with developers. It does not discuss deleting them, beyond saying macOS clears "logs that are safe to delete" when space is needed.

### Cited Findings
- "Delete files in your Downloads folder. Your can open the Downloads folder from the Dock…" (Apple's typo preserved). — [Apple Support 102624](https://support.apple.com/en-us/102624)
- macOS automatically clears "caches and logs that are safe to delete, including … interrupted downloads, staged macOS and app updates…" when space is needed. — [Mac User Guide (macOS 26): Optimize storage](https://support.apple.com/en-us/guide/mac-help/sysp4ee93ca4/26/mac/26)
- **Crash reports on macOS:** "To locate and email crash reports for macOS and Mac Catalyst apps: Open the Console app… Select Crash Reports. Locate crash reports for your app in the list… Right-click the desired log's file name. Select Reveal in Finder." — [Apple Developer: Acquiring crash reports and diagnostic logs](https://developer.apple.com/documentation/xcode/acquiring-crash-reports-and-diagnostic-logs)
- The same doc says "Crash reports commonly have `.crash` and `.ips` file extensions" and that developers rely on customers sending these reports when the Crashes organizer lacks them. — [same](https://developer.apple.com/documentation/xcode/acquiring-crash-reports-and-diagnostic-logs)
- **LEGACY, 2018:** `~/Library/Logs` "Contains log files for the console and specific system services. Users can also view these logs using the Console app." — [macOS Library Directory Details (archive)](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/MacOSXDirectories/MacOSXDirectories.html)

### Inferences
- **(Inferred)** Old `.dmg`/`.pkg` files in Downloads fall under Apple's general "delete files in your Downloads folder" advice. Apple leaves the decision to the user. Nothing in Apple's docs says installers are needed after installation, but nothing endorses deleting them automatically either.
- **(Inferred)** DiagnosticReports (`~/Library/Logs/DiagnosticReports`) are diagnostic evidence that developers and Apple support may ask users for. Deleting them is low risk to system function but destroys troubleshooting data. Apple's position that macOS clears "logs that are safe to delete" suggests the system already manages log retention.

### Gaps
- I found no Apple page that specifically names `.dmg` or `.pkg` installer cleanup.
- I found no Apple page on the retention or rotation policy for `~/Library/Logs/DiagnosticReports`, or on whether deleting it is recommended.

---

## Q6. What does Apple document about URLResourceKey `isPurgeable` and cache-deletion (CacheDelete / `deleted`) behaviour on low disk space?

### Takeaway
Since macOS 11, Foundation has exposed a public `URLResourceKey.isPurgeableKey`: "whether the file system can delete a file when the system needs to free space". The low-space purge mechanism is the `deleted` daemon. Its man page says it tracks purgeable space via registered services, listens for low-space events, and asks clients to purge. Apple documents `volumeAvailableCapacityForImportantUsageKey` as the capacity figure that accounts for this. CacheDelete itself is a private framework with no public developer documentation.

### Cited Findings
- `isPurgeableKey` abstract: "The key for a Boolean value that indicates whether the file system can delete a file when the system needs to free space." Available from macOS 11.0 and iOS 14.0. — [Apple Developer: URLResourceKey.isPurgeableKey](https://developer.apple.com/documentation/foundation/urlresourcekey/ispurgeablekey)
- `URLResourceValues.isPurgeable` abstract: "A Boolean value that indicates whether the file system can delete a file when the system needs to free space." Available from macOS 11.0. — [Apple Developer: URLResourceValues.isPurgeable](https://developer.apple.com/documentation/foundation/urlresourcevalues/ispurgeable)
- `volumeAvailableCapacityForImportantUsageKey`: "Key for the volume's available capacity in bytes for storing important resources (read-only)." Available from macOS 10.13. — [Apple Developer: volumeAvailableCapacityForImportantUsageKey](https://developer.apple.com/documentation/foundation/urlresourcekey/volumeavailablecapacityforimportantusagekey)
- `volumeAvailableCapacityForOpportunisticUsageKey`: "Key for the volume's available capacity in bytes for storing nonessential resources (read-only)." — [Apple Developer: volumeAvailableCapacityForOpportunisticUsageKey](https://developer.apple.com/documentation/foundation/urlresourcekey/volumeavailablecapacityforopportunisticusagekey)
- Both capacity keys carry a privacy note: "This API has the potential of being misused to access device signals to try to identify the device or user, also known as fingerprinting… declare your usage and the reason for using the API in your app or third-party SDK's `PrivacyInfo.xcprivacy` file." — [same](https://developer.apple.com/documentation/foundation/urlresourcekey/volumeavailablecapacityforimportantusagekey)
- *Checking Volume Storage Capacity*: "If you're storing data based on a user request or resources the app requires to function properly… query against volumeAvailableCapacityForImportantUsageKey. However, if you're downloading data in a more predictive manner… query against volumeAvailableCapacityForOpportunisticUsageKey." — [Apple Developer: Checking Volume Storage Capacity](https://developer.apple.com/documentation/foundation/checking-volume-storage-capacity)
- **Local man page** `man 8 deleted`: "deleted – The purgeable space / cache management daemon. deleted is a system daemon that keeps track of purgeable space via registered services. deleted listens for low-space events from the file system and attempts to avoid running out of disk space by requesting that clients purge space. deleted is not intended to be invoked directly." FILES: "/System/Library/CacheDelete — CacheDelete service definitions provided by Apple." The man page is dated "10 September, 2013". — local man page `deleted(8)`, no web URL. `CacheDelete.framework` is present in `/System/Library/PrivateFrameworks` on this machine, so it is a private, undocumented framework.
- Mac User Guide: "Available storage space includes free storage space and purgeable disk caches (disk space might be both available and used at the same time)." — [Mac User Guide (macOS 26): Find and delete files](https://support.apple.com/guide/mac-help/find-and-delete-files-on-your-mac-syspf5a64aa6/26/mac/26)

### Inferences
- **(Inferred)** A cleanup tool can read `isPurgeable` per file to flag items the OS already treats as reclaimable. Deleting such files frees "free" space but not "available" space, because Apple already counts them as available. This is useful for avoiding misleading "space reclaimed" numbers.
- **(Inferred)** Showing both `volumeAvailableCapacityForImportantUsage` and raw free space lets a tool explain purgeable space the way Apple's Storage UI does. Mac App Store distribution would require a privacy-manifest reason for these keys.
- **(Inferred)** Since `deleted` and CacheDelete are private, a third-party tool cannot officially trigger a system purge. The documented path is to let macOS do it under pressure, or `tmutil thinlocalsnapshots` for snapshots.

### Gaps
- The DocC pages for `isPurgeableKey` and `isPurgeable` have only an abstract, with no Discussion. Apple does not document how a file becomes purgeable (setting it is not described), which subsystems mark files purgeable, or the order of purging.
- I found no public Apple developer documentation for CacheDelete, `com.apple.cache_delete`, or the registered-service protocol. Only the 2013 `deleted(8)` man page refers to it.
