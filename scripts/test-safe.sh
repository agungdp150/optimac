#!/usr/bin/env bash
# Until the isolation gate is complete, run only audited focused tests.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test --locked --features fixture-only fixture_recovery
cargo test --locked --features fixture-only recovery_lock
cargo test --locked --features fixture-only recovery_journal
cargo test --locked --features fixture-only --bin optimac configured_inventory_runs_in_menu_without_default_roots
cargo test --locked --features fixture-only fixture_cli_rejects_missing_executable
cargo test --locked --features fixture-only clean_plan
cargo test --locked --features fixture-only counts_metadata_without_following_links_and_honors_limits
cargo test --locked --features fixture-only byte_comparison_is_exact_and_cancellable
cargo test --locked --features fixture-only injected_paths_resolve_consistently_and_missing_home_fails_closed
cargo test --locked --features fixture-only --bin optimac menu_navigation_and_background_actions
