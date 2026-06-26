# Roadmap

OptiMac is being built as a safe macOS maintenance tool with CLI-first workflows and Homebrew distribution.

## Current

- Safe cleanup scanner and executor
- Trash-backed removal with an operation log, `restore`, and `trash empty`
- Configurable cleanup targets and exclude paths via `~/.config/opti-mac/config.json`
- Largest-file analyzer
- Duplicate file finder
- Project artifact purger (node_modules, target, build, Pods, DerivedData, caches, …)
- App uninstaller with bundle ID detection and leftover preview, plus an interactive Uninstall Apps browser in the TUI
- Installed-application listing by size (`apps`)
- Browser privacy cleanup with per-browser targets
- Login item and LaunchAgent listing and toggling
- Homebrew update checker
- Adware/PUP scanner: known signatures plus launch-agent payload heuristics (report-only)
- Hidden-space detection: APFS local snapshots, iOS backups, Docker, and off-Caches app data
- Orphan detection: support data left by uninstalled apps (report-only)
- Doctor: read-only FileVault/SIP/Gatekeeper/firewall/disk/memory check
- Parallel directory sizing and tiered duplicate hashing
- Age-aware cleaning (`--older-than`) and running-process warnings
- Basic system status
- Interactive terminal menu with Catppuccin-style colors, animated loaders, and color-coded sizes
- Homebrew formula template

## Next

- Interactive selection and deletion for artifacts, browser, space, and orphans in the TUI
- App Store / Sparkle update detection beyond Homebrew
- Per-target browser confirmation and profile selection
- One-key snapshot thinning and Docker prune from the space view
- Trash retention policy and automatic pruning

## Later

- Live terminal UI dashboard
- Optional macOS GUI
- Homebrew tap automation and release checksums
