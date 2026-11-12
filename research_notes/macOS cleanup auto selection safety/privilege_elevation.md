# Privilege Elevation for a macOS Disk-Cleanup Tool (CLI/TUI, Rust), macOS 13–26

Scope note: "Local observation" means read-only checks (`ls -ldO`, `man tmutil`, `cat /etc/pam.d/*`, `tmutil listlocalsnapshots /`, `csrutil status`) run on 2026-11-13 on a stock macOS 26.6.2 (build 25G83) machine with SIP enabled, logged in as an admin user. These are a single primary observation, not a web source. Treat them as one data point; ownership and permissions can differ between macOS versions.

## 1. Which cleanup locations need root or Full Disk Access, and is cleaning them worthwhile?

### Takeaway
Almost all space worth reclaiming safely sits in the user's own home (`~/Library/Caches`, app caches, logs), which needs neither root nor sudo. Root-owned system locations (`/Library/Caches/*`, `/private/var/log`, other users' `/private/var/folders`, `/private/var/vm`) need root. Some are also SIP-protected (`/Library/Updates`), so even root cannot delete them. Vendor and how-to sources agree that system caches are "best left alone." The only clearly worthwhile privileged reclaim is Time Machine local snapshots, and Apple's own `tmutil` already handles that. Full Disk Access (TCC) is a separate axis from root: it gates privacy-protected locations, and Unix permissions do not override it.

### Cited Findings
**Ownership and permissions (local observation, macOS 26.6.2, SIP enabled):**
- `/Library/Caches` is `drwxrwxrwt root admin`, so it is world-writable with the sticky bit. A user can only delete entries they own. Its subfolders are owned by `root` or by daemon accounts such as `_iconservices` and `_modelmanagerd`. Listing it as an unprivileged user returned "Operation not permitted" for several `com.apple.*` entries (e.g. `com.apple.aned`), which suggests TCC or SIP protection on top of Unix permissions. — local observation
- `/Library/Logs` is `root wheel 755`, so deleting from it needs root. — local observation
- `/Library/Logs/DiagnosticReports` is `root _analyticsusers 770`, and its `.diag` files are `rw-rw---- root _analyticsusers`. The admin user observed is a member of `_analyticsusers`, so an admin user can probably delete system crash reports without sudo. This is an inference from the modes; deletion was not tested. — local observation
- `/private/var/log` is `root wheel 755`, so root is needed to delete. — local observation
- `/private/var/db/diagnostics` (unified log store) is `root admin 750`: an admin can read it, but only root can delete. — local observation
- `/private/var/folders` is `root wheel 755` with the `sunlnk` flag. Per-user subfolders sit under it, and system and daemon folders (`zz/…`) are root-owned. Another user's folder needs root. — local observation. Wardle explains that Apple added the SIP `sunlnk` flag to `$TMPDIR` (which lives under `/var/folders`) to block symlink attacks — [Objective-See blog 0x55](https://objective-see.org/blog/blog_0x55.html)
- `/Library/Updates` is `root wheel 755` with the `restricted` flag, which marks it as SIP-protected (flag observed locally). SIP was enabled. — local observation (that `restricted` means SIP-protected is standard macOS behaviour, but no web source was fetched for it)
- `/private/var/vm` holds a root-owned `sleepimage` (2 GB on the test machine, mode 1600). — local observation

**Full Disk Access (TCC) vs root:**
- TCC first checks whether the process holds Full Disk Access (`kTCCServiceSystemPolicyAllFiles`). If not, it checks per-location services (Desktop, Documents, Downloads, removable, network and FileProvider volumes), and if none apply it shows a consent dialog. — [Eclectic Light, "Privacy: How locations are protected" (2026-04-20)](https://eclecticlight.co/2026/04/20/privacy-how-locations-are-protected/)
- "You can have full Unix-style permissions to read and write a file, but privacy protection can still block that." — [Eclectic Light (2026-04-20)](https://eclecticlight.co/2026/04/20/privacy-how-locations-are-protected/)
- "There are also subtle differences between protected locations that can make generalisation unreliable." Eclectic Light's macOS 26.4 table is published as an image. — [Eclectic Light (2026-04-20)](https://eclecticlight.co/2026/04/20/privacy-how-locations-are-protected/); also [Eclectic Light, "Files & Folders or Full Disk Access?" (2026-04-08)](https://eclecticlight.co/2026/04/08/privacy-files-folders-or-full-disk-access/)
- Escalating privileges "does not bypass all privilege restrictions." — Apple DTS (Quinn), [BSD Privilege Escalation on macOS](https://developer.apple.com/forums/thread/708765)
- Any app that can control Finder (granted under Automation) effectively has Full Disk Access, even though it is not listed in the FDA pane. — [SentinelOne Labs, TCC bypasses](https://www.sentinelone.com/de/labs/bypassing-macos-tcc-user-privacy-protections-by-accident-and-design/)

**Time Machine local snapshots:**
- The `tmutil` man page (macOS 26.6.2) says: "Several, but not all, verbs require root and Full Disk Access privileges. Full Disk Access privileges can be granted to the Terminal application used to run tmutil." The page does not say whether `deletelocalsnapshots` or `thinlocalsnapshots` need root. — local observation (`man tmutil`)
- On macOS 26.6.2, `deletelocalsnapshots {mount_point | date}` accepts either a date (`YYYY-MM-DD-HHMMSS`) or a disk. `thinlocalsnapshots mount_point [purge_amount] [urgency]` lets macOS choose which snapshots to remove. — local observation (`man tmutil`). Older documentation and guides show only the date form — [Apple Developer Forums 708921](https://developer.apple.com/forums/thread/708921), [Apple Community 255461563](https://discussions.apple.com/thread/255461563). The mount-point form is therefore version-dependent, not an error.
- `tmutil listlocalsnapshots /` ran without sudo. — local observation
- Community guides run deletion with `sudo`. — [Apple Community 255461563](https://discussions.apple.com/thread/255461563)
- Local snapshots are treated as purgeable (deleted can remove them at any time). Howard Oakley discusses why purging can still fail and recommends `thinlocalsnapshots` so macOS decides what to delete. — [Eclectic Light, "Aren't snapshots purgeable?" (2026-08-24)](https://eclecticlight.co/2026/08/24/arent-snapshots-purgeable/); [Eclectic Light, "Why can purging snapshots fail?" (2026-08-27)](https://eclecticlight.co/2026/08/27/why-can-purging-snapshots-fail/); [Eclectic Light, "Take control of snapshots" (2026-09-09)](https://eclecticlight.co/2026/09/09/take-control-of-snapshots/)

**Is cleaning root-owned caches worthwhile?**
- Even CleanMyMac (MacPaw), which sells a cleaner, says that `~/Library/Caches` is "usually safe to delete," while "System cache lives in /Library/Caches or /System/Library/Caches; it is best to leave these alone unless you're troubleshooting something specific." — [CleanMyMac blog](https://cleanmymac.com/blog/clear-cache-mac) (vendor source)
- Deleting caches while an app is running can make it crash, and apps load more slowly while caches rebuild. — [Norton community thread](https://community.norton.com/t/clean-my-mac/512781) (forum source, low authority)
- Clearing the system cache "will not speed up your Mac." — [wikiHow mirror](https://kiwix.lema.org/wikihow_en_maxi_2023-03/Is-It-Safe-to-Delete-Cache-Files-on-Mac) (low authority)
- An exploited cleaner helper function named `removeDiagnosticLogs` shows that commercial cleaners did implement root deletion of system logs, and that these root code paths became attack surface (see section 3). — [Talos reports index via TALOS-2018-0709](https://talosintelligence.com/vulnerability_reports/TALOS-2018-0709); [NVD CVE-2019-5011](https://nvd.nist.gov/vuln/detail/cve-2019-5011)

### Inferences
- For a cleanup tool, root unlocks little beyond (a) system logs and crash reports, which are usually small, (b) root-owned `/Library/Caches` entries, which experts advise leaving alone, and (c) Time Machine snapshots. `tmutil` covers (c) natively. The cost/benefit argues for running unprivileged by default.
- SIP-protected paths (`/Library/Updates`, flagged `restricted`) are pointless targets: root cannot delete them while SIP is on, and macOS manages them itself.
- `/private/var/vm` (swap and `sleepimage`) is managed by the OS. Deleting from it is not a sensible cleanup action. No sourced expert statement was found, so this rests on the local observation of root ownership and general OS-management behaviour.
- The unified log (`/private/var/db/diagnostics`) and `/private/var/log` hold forensic and troubleshooting evidence. Deleting them is a security and support cost, not just a space gain.
- FDA should be scoped separately: a CLI under Terminal inherits Terminal's TCC grants. Asking the user to grant Terminal FDA widens access for everything run in that Terminal. This is an inference from the man page and the TCC model above.

### Gaps
- No authoritative Apple per-path table of root vs TCC vs SIP requirements was found. Eclectic Light's macOS 26.4 table is an image and was not extracted.
- Could not confirm from a web source whether `tmutil deletelocalsnapshots` needs root or Terminal FDA on macOS 13–26. The man page is silent, and community examples use sudo. Testing on each target macOS version is needed.
- Eclectic Light had no direct article on cleaning `/Library/Caches` (search returned none). Typical byte sizes of `/Library/Caches`, `/private/var/log` and DiagnosticReports were not found.
- No sourced statement was found on deleting `/private/var/db/diagnostics` (the unified log) versus using `log erase`.

## 2. Apple-sanctioned elevation mechanisms and their trade-offs (macOS 13–26)

### Takeaway
Apple DTS's canonical guidance (Quinn, "BSD Privilege Escalation on macOS," first posted 2022-06-22 and revised through 2025-03-24) is as follows:
- Never use setuid-root.
- Don't use `AuthorizationExecuteWithPrivileges` (deprecated since 10.7) in distributed products.
- `SMJobBless` is deprecated since macOS 13.
- `SMAppService` (macOS 13+) is the recommended path for ongoing privileges.
- `sudo`, `authopen` and `osascript` are for interactive use, not for use as an API.

For a terminal tool, the natural fit is the user running the native command (`sudo <specific command>`) themselves. macOS's own sudo prompt handles authentication there, and Touch ID is possible via `/etc/pam.d/sudo_local`.

### Cited Findings
**DTS option table** — all from Quinn, [Apple Developer Forums 708765](https://developer.apple.com/forums/thread/708765) (first posted 2022-06-22; revised 2024-07-29, 2024-11-15 and 2025-03-24):
- Setuid-root: "Do not use a setuid-root executable. Ever… Doing that is creating a security vulnerability looking for an attacker to exploit it."
- `sudo`, `authopen` and `osascript` are interactive use only: "These are not appropriate to use as API… while it may be possible to invoke `sudo` programmatically under some circumstances, by the time you're done you'll have code that's way more complicated than the alternatives."
- `AuthorizationExecuteWithPrivileges` has been **deprecated since macOS 10.7 (2011)**: "While `AuthorizationExecuteWithPrivileges` still works, it's been deprecated for many years. Do not use it in a widely distributed product."
- AppleScript `do shell script … with administrator privileges` (or `osascript`) is acceptable for "an ad hoc solution to distribute to a limited audience, and you need one-shot privileges."
- `SMJobSubmit` (system domain) has been **deprecated since macOS 10.10**, but is "used by the very popular Sparkle update framework, and thus it's unlikely to break without warning." DTS calls it reasonable for one-shot privileges.
- `SMJobBless` has been **deprecated since macOS 13 (2022)**. Use it only for ongoing privileges on older systems.
- `SMAppService` (introduced in macOS 13) is the recommended path for ongoing privileges without an installer package.
- An installer package is "by far the easiest solution" when privileges are only needed at install time.
- Further guidance: "Do not run GUI code as root." DTS also suggests a custom Authorization Services right so admins can restrict privileged operations.

**Corroboration and status reports:**
- `AuthorizationExecuteWithPrivileges` is deprecated since 10.7 but still present in macOS 13. — [STPrivilegedTask README (via awesome.ecosyste.ms)](https://awesome.ecosyste.ms/projects/github.com%2Fsveinbjornt%2FSTPrivilegedTask). One JUCE user reported it stopped working with the Xcode 14.3 SDK — [JUCE forum](https://forum.juce.com/t/elevated-permissions-in-macos-or-so-we-thought-edited/55419) (single-source)
- A single pull-request report says that on macOS 26.4.1, `smd` rejects `SMJobBless` installs that previously worked. — [exelban/stats PR 3194 mirror](https://github.kakarurec.workers.dev/exelban/stats/pull/3194) (**single-source, unverified**)
- Ongoing migration discussions: [Migrating away from SMJobBless (forums 725811)](https://developer.apple.com/forums/thread/725811), [forums 817343](https://developer.apple.com/forums/thread/817343). One developer found that an `SMAppService` daemon could not write somewhere the `SMJobBless` helper could — [forums 762901](https://developer.apple.com/forums/thread/762901)
- A one-time-escalation thread (Nov 2024) led DTS to add a "new option" (Sparkle's approach) to the canonical post. The asker described the `osascript` dialog as "shady" and noted that the right `com.apple.ServiceManagement.daemons.modify` gives a more specific prompt ("is trying to add a new helper tool"). — [Apple Developer Forums 768765](https://www.developer.apple.com/forums/thread/768765)

**SMAppService details:**
- Introduced in macOS 13 Ventura to replace `SMJobBless` and `SMLoginItemSetEnabled`. Registering a daemon is two-step: the user approves a background item, and then authenticates because the daemon runs as root. Status values are NotRegistered, Enabled, RequiresApproval and NotFound. Daemon plists and executables live inside the app bundle. A user who disables the item in System Settings removes it from launchd. — [Csaba Fitzl (theevilbit), "The SMAppService API – Quick Notes" (2023-09-28)](https://theevilbit.github.io/posts/smappservice)
- After `register()`, status can be `requiresApproval` until the user enables the daemon in System Settings → Login Items. `openSystemSettingsLoginItems()` opens that pane. — [forums 721919](https://developer.apple.com/forums/thread/721919), [Apple SMAppService docs](https://developer.apple.com/documentation/servicemanagement/smappservice.md)
- The plist lives in `Contents/Library/LaunchDaemons/` inside the app bundle. Developers report missing prompts and stale Login Items entries. — [forums 707482](https://developer.apple.com/forums/thread/707482), [forums 721919](https://developer.apple.com/forums/thread/721919)
- A Rust binding crate `smappservice-rs` exists. — [rust-digger listing](https://rust-digger.code-maven.com/crates/smappservice-rs) (single-source; maturity not assessed)

**sudo and Touch ID:**
- Starting with macOS Sonoma (14), `/etc/pam.d/sudo` includes `sudo_local`. Apple ships `sudo_local.template` with `pam_tid.so` commented out. Copying the template to `sudo_local` and uncommenting that line enables Touch ID for sudo, and the change survives OS updates. Earlier edits to `/etc/pam.d/sudo` were overwritten by updates. — [Six Colors (2023-08)](https://sixcolors.com/post/2023/08/in-macos-sonoma-touch-id-for-sudo-can-survive-updates/)
- Confirmed on macOS 26.6.2: `/etc/pam.d/sudo` begins with `auth include sudo_local`, then `pam_smartcard.so` and `pam_opendirectory.so`. `sudo_local.template` reads "local config file which survives system update and is included for sudo," with `#auth sufficient pam_tid.so`. — local observation
- The `sudo_local` route works only at the local machine (not over SSH). — [Six Colors](https://sixcolors.com/post/2023/08/in-macos-sonoma-touch-id-for-sudo-can-survive-updates/); see also [1Password community PSA](https://www.1password.community/developers-69/psa-macos-sonoma-and-sudo-biometrics-12789)

### Inferences
- For a Rust CLI/TUI that is not an app bundle, `SMAppService` is a poor fit. It expects a bundled, signed app with an embedded daemon plist and user approval in System Settings, and it installs a persistent root daemon. That is heavy machinery and a permanent attack surface for an occasional cleanup task. (Inference from the Fitzl and Apple sources above.)
- `AuthorizationExecuteWithPrivileges` should be ruled out: it has been deprecated for about 15 years and has a history of root exploits (section 3). `SMJobBless` is deprecated and reportedly breaking on 26.x (single source).
- An `osascript … with administrator privileges` dialog is DTS-tolerated only for ad hoc, limited-audience tools. It is also visually the same dialog malware fakes (section 3), so it trains the wrong habit.
- In a terminal, the most native, least-privilege option is to print the exact command and let the user run it, for example `sudo tmutil thinlocalsnapshots / …` or `sudo rm …` on a specific reviewed path. macOS's own sudo/PAM stack then handles authentication, including Touch ID if the user enabled `sudo_local`. A middle ground is for the tool to exec `sudo` with an inherited TTY for one specific command, leaving the prompt to sudo. This avoids collecting the password but should be limited to a fixed, reviewed argv. (Inference; DTS says sudo is "interactive use", which this respects.)

### Gaps
- The Apple Platform Security Guide pages on SIP and TCC were not fetched, so there is no direct quote on root vs SIP.
- Apple's official `SMAppService` status documentation was not fetched in full; status semantics come from Fitzl and forum posts.
- Not sourced: sudo credential caching on macOS (`timestamp_timeout` default, per-tty tickets). Check `man sudoers` on the target OS.
- Not sourced: Touch ID for sudo inside tmux or screen (commonly needs `pam_reattach`). This matters for TUI users.
- Whether `SMJobBless` is actually blocked on macOS 26.4+ rests on one GitHub report.

## 3. Expert opinion and evidence on apps collecting the password themselves, piping to sudo, and privileged-helper bugs

### Takeaway
Collecting the admin password in the app's own UI and piping it to `sudo -S` exposes credentials (CWE-522 and CWE-256 apply directly to a password held in memory or files). It also reproduces exactly the pattern macOS infostealers use in 2024–2026: fake password dialogs, validated in a loop. Privileged helpers in Mac cleaners have a documented record of root-escalation CVEs, often because the root helper did not validate its caller. The secure pattern is to never touch the password, run unprivileged, and delegate any single reviewed privileged action to the OS's own authentication (the sudo TTY prompt or the system dialog).

### Cited Findings
**Credential-handling weaknesses (CWE):**
- CWE-522 "Insufficiently Protected Credentials": "The product transmits or stores authentication credentials, but it uses an insecure method that is susceptible to unauthorized interception and/or retrieval." Mitigation: "Use industry standards to protect the credentials (e.g. LDAP, keystore, etc.)." — [MITRE CWE-522](https://cwe.mitre.org/data/definitions/522.html)
- CWE-256 "Plaintext Storage of a Password": "The product stores a password in plaintext within resources such as memory or files." Mitigation: "Avoid storing passwords in easily accessible locations." — [MITRE CWE-256](https://cwe.mitre.org/data/definitions/256.html)
- Note: no source found explicitly maps `sudo -S` to these CWEs. The mapping is an inference (see Inferences).

**`sudo -S` and password piping:**
- sudo reads from the terminal by default. `-S` exists so sudo can take the password from stdin via a pipe. A 2006 sudo-workers thread flagged a related risk: when stdin is used, the password can leak into the sudoed command's input. — [sudo-workers 2006](https://sudo.ws/pipermail/sudo-workers/2006-August/000437.html); [sudo(8) man page](https://manpages.debian.org/jessie/sudo/sudo.8.en.html)
- Passing passwords on the command line or in scripts leaves them in shell history or on disk in the script file. — [Baeldung, sudo non-interactive mode](https://www.baeldung.com/linux/sudo-non-interactive-mode)
- In 2003, an Apple mailing-list thread on `do shell script` with an embedded password warned that any user can run `ps`, so a password visible in arguments can be exposed. — [applescript-users 2003](https://lists.apple.com/archives/applescript-users/2003/Apr/msg00028.html)
- DTS: invoking sudo programmatically yields code "way more complicated than the alternatives." — [forums 708765](https://developer.apple.com/forums/thread/708765)

**Phishing-pattern training (password dialogs are the #1 macOS stealer technique):**
- Patrick Wardle (Objective-See) analyses macOS stealers that prompt for the user's password with a fake dialog. — [Objective-See blog 0x88](https://objective-see.org/blog/blog_0x88.html)
- ClickLock stealer (2026): it starts with an `osascript` fake password dialog and validates the input with `dscl /Local/Default -authonly`. If the user refuses, it kills apps in a loop until the password is entered. — [Group-IB](https://group-ib.com/blog/clicklock-stealer-macos-malware); [The Hacker News (2026-07)](https://thehackernews.com/2026/07/new-clicklock-macos-stealer-kills-apps.html)
- SHub Reaper: the dialog validates the password in real time and reappears if it is wrong. — [SentinelOne](https://sentinelone.com/blog/shub-reaper-macos-stealer-spoofs-apple-google-and-microsoft-in-a-single-attack-chain)
- AppleScript/ClickFix campaigns deliver stealers via pasted Terminal commands, and Apple has added new Terminal protections. — [Netskope](https://netskope.com/blog/macos-clickfix-campaign-applescript-stealers-new-terminal-protections); [The Register (2026-04-21)](https://www.theregister.com/2026/04/21/macos_clickfix_attacks_deliver_applescript/)
- Defenders publish rules to block `osascript` password dialogs, noting that legitimate IT scripts sometimes use them. — [North Pole Security cookbook](https://northpole.security/cookbook/block-fake-password-prompts)
- Advice in that coverage: a password dialog that appears after running a copied command "does not become trustworthy merely because it resembles a native macOS prompt." — [North Pole Security](https://northpole.security/cookbook/block-fake-password-prompts) / search-summary attribution; treat the exact wording as unverified.

**Privileged-helper vulnerabilities in cleaners and utilities:**
- CleanMyMac X 4.04: Cisco Talos found many root-helper functions callable by any app. TALOS-2018-0709 / CVE-2018-4036 (`removeKextAtPath`) states: "There is no validation of the calling application. Therefore, any application is able to access this function, and because this is a privileged helper, it runs as root." CVSS 7.1. Disclosed to the vendor 2018-11-09, patched 2018-12-27, public 2019-01-02. — [Talos TALOS-2018-0709](https://talosintelligence.com/vulnerability_reports/TALOS-2018-0709)
- Sibling Talos reports (TALOS-2018-0705/0706/0707/0710/0716/0718/0719; CVE-2018-4032 through 4044) cover functions including `moveItemAtPath`, `removeDiagnosticLogs` and `moveToTrashItemAtPath`. — [TALOS-2018-0705](https://talosintelligence.com/vulnerability_reports/TALOS-2018-0705), [TALOS-2018-0718](https://talosintelligence.com/vulnerability_reports/TALOS-2018-0718), [TALOS-2018-0719](https://talosintelligence.com/vulnerability_reports/TALOS-2018-0719)
- CVE-2019-5011: privilege escalation in the CleanMyMac X 4.20 helper "due to improper updating" (incomplete cleanup). — [NVD CVE-2019-5011](https://nvd.nist.gov/vuln/detail/cve-2019-5011)
- Wojciech Reguła (SecuRing), "Abusing & Securing XPC in macOS apps" (Objective by the Sea v3, 2020): abusing XPC led to many severe bugs. The typical flaw is an XPC server with no client validation, or clients with weak hardened-runtime protections that allow injection and impersonation. He open-sourced a secure helper example. — [SecuRing presentation](https://www.securing.pl/en/presentation/abusing-securing-xpc-in-macos-apps/); [slides (papers.put.as)](https://papers.put.as/papers/macosx/2020/OBTS_v3_wRegula.pdf)
- Reguła and Csaba Fitzl, "Exploiting XPC in AntiVirus" (NullCon 2021), and Fitzl, "XPC exploitation on macOS" (Hacktivity 2020): the parent-app/helper trust mechanism is abusable (CVE-2019-13013), and developers often omit permission checks on Mach services. — [NullCon 2021 slides](https://theevilbit.github.io/talks_workshops/2021/NullCon2021-Regula-Fitzl-Exploiting-XPC-in-AntiVirus.pdf); [Hacktivity 2020 slides](https://theevilbit.github.io/talks_workshops/2020/Hacktivity2020-Fitzl-XPC-exploitation-on-macOS.pdf)
- Reguła: a malicious helper can trick TCC into treating the wrong app as "responsible," letting it use that app's TCC grants (a TCC bypass via privileged helpers). — [Michael Tsai summary (2021-12-23)](https://mjtsai.com/blog/2021/12/23/bypass-tcc-via-privileged-helpers)
- Other privileged-helper LPEs in Mac utilities: Intego ([Quarkslab](https://blog.quarkslab.com/intego_lpe_macos_1.html)), IPVanish VPN ([SecureLayer7](https://blog.securelayer7.net/ipvanish-vpn-macos-privilege-escalation/)), Zoom updater ([Sophos](https://www.sophos.com/en-gb/blog/zoom-for-mac-patches-get-root-bug-update-now)), and Apple's own StorePrivilegedTaskService ([Computest Sector 7](https://sector7.computest.nl/post/2021-12-storeprivilegedtaskservice)).

**AuthorizationExecuteWithPrivileges abuse:**
- Wardle (2020-03-16) covers CVE-2017-7170. The API "does not validate the binary that will be executed (as root!)." Its externalized authorization reference was written to a temp file that an unprivileged user could read, then reused to run arbitrary commands as root after the user authenticated at a legitimate prompt. Wardle says the flaw existed from OS X 10.4 until the 10.13.1 fix, and that "pretty much every 3rd-party program" needing privileges used the API. — [Objective-See blog 0x55](https://objective-see.org/blog/blog_0x55.html)
- Wardle's DEF CON 25 (2017) talk "Death by 1000 Installers" argued that privileged installers widely call insecure APIs such as AEWP, which runs a binary that a local attacker can swap during a race. — [SpeakerDeck slides](https://speakerdeck.com/patrickwardle/defcon-2017-death-by-1000-installers-its-all-broken); [Threatpost](https://threatpost.com/deprecated-insecure-apple-authorization-api-can-be-abused-to-run-code-at-root/127618); [Jamf mitigation blog](https://www.jamf.com/blog/mitigating-the-risks-of-authorizationexecutewithprivileges-and-software-installers/)

### Inferences
- A Rust TUI that renders its own "Enter password" field reproduces the stealer UX that defenders now block and that users are being trained to distrust. It holds the password in process memory (CWE-256), and piping it to `sudo -S` adds the stdin-leak and caching concerns (CWE-522). Rust's memory safety does not address any of this. Zeroizing buffers (e.g. the `zeroize` crate) reduces but does not remove exposure, and the trust and phishing problem remains regardless.
- After a successful `sudo`, the sudo timestamp means later `sudo` calls from the same TTY may not prompt (the default timeout was not sourced, see Gaps). A tool that runs sudo for the user may silently leave an elevated window that other processes in that terminal can use.
- The cleaner CVEs show that the dangerous part is a generic root primitive ("delete/move this path") exposed to callers. If a privileged step exists, it should take no arbitrary paths, should validate against an allowlist of fixed targets, should resolve paths with no-follow semantics (to avoid symlink and TOCTOU issues), and should be tiny and auditable.
- Recommended least-privilege design for this tool, synthesised from DTS guidance plus the CVE history:
  1. Run unprivileged by default and only clean user-owned locations.
  2. For privileged items, list them as "requires admin" with exact size and command, and do not include them in automatic selection.
  3. Prefer printing the native command for the user to run (`sudo tmutil thinlocalsnapshots / <bytes> 4`, or `tmutil deletelocalsnapshots <date>`).
  4. If the tool runs it, exec `sudo` with a fixed argv on the user's real TTY so sudo/PAM prompts (Touch ID if enabled), and never read, store, log or pipe the password.
  5. Never install a persistent root helper for a cleanup tool.

### Gaps
- No OWASP or CIS Benchmark text was retrieved on applications collecting OS passwords. The CIS macOS benchmark's sudo timeout recommendation was not verified.
- No Apple Platform Security Guide quote was obtained on system authentication dialogs or why apps should not collect passwords.
- No source specifically about Rust CLI/TUI password handling was found. Rust-specific points are inference.
- No 2023–2026 CVEs in cleaner privileged helpers surfaced. Results were dominated by 2018–2019 CleanMyMac X reports, so it is unknown whether newer cleaner helpers have been audited.
- The specific Objective-See blog 0x88 content (which stealer, date) was not fetched; it is cited only as a search result about fake password prompts.
