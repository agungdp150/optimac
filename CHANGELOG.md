# 0.2.0 (unreleased)

Breaking: removed Go source and toolchain. Default builds now produce the Rust
`optimac` binary. Includes the cat-themed menu, metadata and large-file inspection, duplicate
comparison, artifact discovery and read-only system diagnostics. The menu supports confirmed developer-cache cleanup, owned app-bundle removal
and non-overwriting Rust recovery. CLI mutation flags remain disabled. Existing configuration and recovery data are untouched.
The Rust UI supports resizing and cancellation; scans have bounded traversal.
Homebrew uses HEAD until a Rust release archive/checksum is available.

# Changelog

## 0.1.2 - 2026-06-28

- Fixed release metadata after the `optimac` rename: README badge, default CLI version, and release checks now stay aligned with `VERSION`.

## 0.1.1 - 2026-06-28

- Renamed the installable command and Homebrew formula from `opti-mac` to `optimac` to match the GitHub repository name.

## 0.1.0 - 2026-06-26

- Initial tracked OptiMac CLI/TUI version.
- Added safe deep clean, analyze disk, duplicate review, app uninstall, system status, trash/restore, and Homebrew packaging foundation.
