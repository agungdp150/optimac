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
