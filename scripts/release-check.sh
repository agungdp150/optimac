#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version="$(tr -d '[:space:]' < VERSION)"
actual="$(cargo run --quiet --locked -- version)"
[[ "$actual" == "$version" ]] || { echo 'Version mismatch' >&2; exit 1; }
if [[ -n "${GITHUB_REF_NAME:-}" && "${GITHUB_REF_TYPE:-}" == tag ]]; then
  [[ "$GITHUB_REF_NAME" == "v$version" ]] || { echo 'Tag/version mismatch' >&2; exit 1; }
fi
cmp Formula/optimac.rb packaging/homebrew/optimac.rb
[[ ! -f go.mod && ! -d internal && ! -d cmd ]]
printf 'release-check: ok\n'
