#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  printf 'security-scan: %s\n' "$1" >&2
  exit 1
}

tracked_files() {
  find . \
    -path './.git' -prune -o \
    -path './.cache' -prune -o \
    -path './bin' -prune -o \
    -path './vendor' -prune -o \
    -type f \
    \( -name '*.go' -o -name '*.sh' -o -name '*.rb' -o -name '*.md' -o -name '*.yml' -o -name '*.yaml' -o -name '*.json' -o -name '*.svg' -o -name 'Makefile' \) \
    -print
}

is_allowed_url() {
  case "$1" in
    https://github.com/luceid/optimac|https://github.com/luceid/optimac/*|https://img.shields.io/*|https://www.w3.org/*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

scan_urls() {
  local bad=0
  while IFS= read -r file; do
    case "$file" in
      *_test.go)
        continue
        ;;
    esac
    while IFS= read -r url; do
      [ -z "$url" ] && continue
      if ! is_allowed_url "$url"; then
        printf 'Unapproved external URL in %s: %s\n' "$file" "$url" >&2
        bad=1
      fi
    done < <(grep -Eo 'https?://[A-Za-z0-9._~:/?#\[\]@!$&'"'"'()*+,;=%-]+' "$file" 2>/dev/null || true)
  done < <(tracked_files)
  [ "$bad" -eq 0 ] || fail "external URLs must be allowlisted in scripts/security-scan.sh"
}

scan_suspicious_patterns() {
  local bad=0
  while IFS=: read -r file line text; do
    case "$file" in
      ./internal/opti/threats.go|*_test.go)
        continue
        ;;
    esac
    printf 'Suspicious download/execute pattern in %s:%s: %s\n' "$file" "$line" "$text" >&2
    bad=1
  done < <(grep -RInE '(curl|wget)[^|;&]*[|][[:space:]]*(sh|bash|zsh)|eval[[:space:]].*(curl|wget)|base64[[:space:]]+(-d|--decode).*[|][[:space:]]*(sh|bash|zsh)' . \
    --exclude-dir=.git --exclude-dir=.cache --exclude-dir=bin --exclude-dir=vendor 2>/dev/null || true)

  while IFS=: read -r file line text; do
    case "$file" in
      ./internal/opti/admin.go|./internal/opti/optimize.go|./internal/opti/threats.go|./scripts/security-scan.sh)
        continue
        ;;
    esac
    printf 'Unreviewed administrator AppleScript in %s:%s: %s\n' "$file" "$line" "$text" >&2
    bad=1
  done < <(grep -RInE 'with administrator privileges|osascript.*-e' . \
    --include='*.go' --include='*.sh' --exclude='*_test.go' --exclude-dir=.git --exclude-dir=.cache --exclude-dir=bin --exclude-dir=vendor 2>/dev/null || true)

  while IFS=: read -r file line text; do
    printf 'Shell command execution needs security review in %s:%s: %s\n' "$file" "$line" "$text" >&2
    bad=1
  done < <(grep -RInE 'exec\.Command(Context)?\([[:space:]]*"(sh|bash|zsh)"[[:space:]]*,[[:space:]]*"-c"|/bin/(sh|bash|zsh)[[:space:]]+-c' . \
    --include='*.go' --include='*.sh' --exclude='*_test.go' --exclude-dir=.git --exclude-dir=.cache --exclude-dir=bin --exclude-dir=vendor 2>/dev/null || true)

  while IFS=: read -r file line text; do
    printf 'Network client API needs security review in %s:%s: %s\n' "$file" "$line" "$text" >&2
    bad=1
  done < <(grep -RInE '"net/http"|http\.(Get|Post|PostForm|NewRequest|DefaultClient)|websocket|grpc\.Dial' . \
    --include='*.go' --exclude='*_test.go' --exclude-dir=.git --exclude-dir=.cache --exclude-dir=bin --exclude-dir=vendor 2>/dev/null || true)

  while IFS=: read -r file line text; do
    printf 'Potential credential capture UI in %s:%s: %s\n' "$file" "$line" "$text" >&2
    bad=1
  done < <(grep -RInE '<form|type=["'\'']password["'\'']|name=["'\''](password|token|api[_-]?key)["'\'']' . \
    --include='*.md' --include='*.html' --include='*.svg' --exclude-dir=.git --exclude-dir=.cache --exclude-dir=bin --exclude-dir=vendor 2>/dev/null || true)

  [ "$bad" -eq 0 ] || fail "suspicious security-sensitive patterns found"
}

scan_urls
scan_suspicious_patterns
printf 'security-scan: ok\n'
