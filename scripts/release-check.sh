#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
README="$ROOT/README.md"
VERSION_FILE="$ROOT/VERSION"
MAIN="$ROOT/cmd/optimac/main.go"
FORMULA="$ROOT/Formula/optimac.rb"
PACKAGED_FORMULA="$ROOT/packaging/homebrew/optimac.rb"

fail() {
  printf 'release-check: %s\n' "$1" >&2
  exit 1
}

extract_value() {
  local key="$1"
  local file="$2"
  awk -v key="$key" '$1 == key { gsub(/"/, "", $2); print $2; exit }' "$file"
}

[[ -f "$FORMULA" ]] || fail "missing Formula/optimac.rb"
[[ -f "$PACKAGED_FORMULA" ]] || fail "missing packaging/homebrew/optimac.rb"
[[ -f "$VERSION_FILE" ]] || fail "missing VERSION"
[[ -f "$MAIN" ]] || fail "missing cmd/optimac/main.go"

version="$(tr -d '[:space:]' < "$VERSION_FILE")"
[[ -n "$version" ]] || fail "VERSION is empty"

grep -q "version-${version}-" "$README" ||
  fail "README version badge does not match VERSION ($version)"

grep -q "var version = \"$version\"" "$MAIN" ||
  fail "cmd/optimac/main.go default version does not match VERSION ($version)"

root_url="$(extract_value url "$FORMULA")"
packaged_url="$(extract_value url "$PACKAGED_FORMULA")"
[[ -n "$root_url" ]] || fail "Formula/optimac.rb is missing url"
[[ "$root_url" == "$packaged_url" ]] || fail "formula URLs differ"

root_sha="$(extract_value sha256 "$FORMULA")"
packaged_sha="$(extract_value sha256 "$PACKAGED_FORMULA")"
[[ -n "$root_sha" ]] || fail "Formula/optimac.rb is missing sha256"
[[ "$root_sha" != "REPLACE_WITH_RELEASE_SHA256" ]] || fail "formula sha256 is still a placeholder"
[[ "$root_sha" == "$packaged_sha" ]] || fail "formula SHA256 values differ"

grep -q 'brew tap agungdp150/optimac https://github.com/agungdp150/optimac' "$README" ||
  fail "README is missing the working direct tap command"

if grep -q 'brew install agungdp150/tap/optimac' "$README"; then
  if ! git ls-remote https://github.com/agungdp150/homebrew-tap >/dev/null 2>&1; then
    fail "README advertises agungdp150/tap, but agungdp150/homebrew-tap does not exist"
  fi
fi

printf 'release-check: ok\n'
