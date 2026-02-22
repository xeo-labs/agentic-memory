#!/usr/bin/env bash
set -euo pipefail

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

find_fixed() {
  local pattern="$1"
  shift
  if command -v rg >/dev/null 2>&1; then
    rg -nF "$pattern" "$@"
  else
    grep -R -n -F -- "$pattern" "$@"
  fi
}

assert_file() {
  [ -f "$1" ] || fail "Missing required file: $1"
}

assert_contains() {
  local pattern="$1"
  shift
  find_fixed "$pattern" "$@" >/dev/null || fail "Missing required pattern: ${pattern}"
}

assert_one_of() {
  local matched=0
  for candidate in "$@"; do
    if [ -f "$candidate" ]; then
      matched=1
      break
    fi
  done
  [ "$matched" -eq 1 ] || fail "Missing required file (one of): $*"
}

assert_not_tracked() {
  local path="$1"
  if git ls-files --error-unmatch "$path" >/dev/null 2>&1; then
    fail "Internal-only file must not be tracked: $path"
  fi
}

assert_no_tracked_prefix() {
  local pattern="$1"
  if [ -n "$(git ls-files "$pattern")" ]; then
    fail "Internal-only path must not be tracked: $pattern"
  fi
}

assert_image_spacing() {
  local min_gap=10
  local prev=0
  local line
  while IFS= read -r line; do
    local n="${line%%:*}"
    if [ "$prev" -ne 0 ] && [ $((n - prev)) -lt "$min_gap" ]; then
      fail "README image blocks too close together (lines ${prev} and ${n})"
    fi
    prev="$n"
  done < <(grep -n '<img src="assets/' README.md || true)
}

assert_file "docs/ecosystem/CANONICAL_SISTER_KIT.md"
assert_file "scripts/install.sh"
assert_file "scripts/check-install-commands.sh"
assert_file "docs/quickstart.md"
assert_file "docs/concepts.md"
assert_file "docs/integration-guide.md"
assert_file "docs/faq.md"
assert_file "docs/benchmarks.md"
assert_file "docs/api-reference.md"
assert_one_of "docs/file-format.md" "docs/LIMITATIONS.md"
assert_not_tracked "ECOSYSTEM-CONVENTIONS.md"
assert_no_tracked_prefix "docs/internal/*"

assert_contains '## 1. Release Artifact Contract' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '## 2. Install Contract Spec' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '## 3. Reusable CI Guardrails' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '## 4. README Canonical Layout' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '## 5. MCP Canonical Profile' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '## 6. Packaging Policy' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '## 7. Versioning and Release Policy' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '## 8. Design Asset Contract' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '## 9. Env Var Namespace Contract' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '## 10. New-Sister Bootstrap' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '## 11. Workspace Orchestrator Contract' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'https://agentralabs.tech/docs/ecosystem-feature-reference' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'https://agentralabs.tech/docs/sister-docs-catalog' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'docs folder required for every sister' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'web docs wiring is mandatory before release' docs/ecosystem/CANONICAL_SISTER_KIT.md

assert_contains '<img src="assets/github-hero-pane.svg"' README.md
assert_contains '<img src="assets/github-terminal-pane.svg"' README.md
assert_contains '## Install' README.md
assert_contains '## Quickstart' README.md
assert_contains '## How It Works' README.md
assert_image_spacing

assert_contains 'MCP client summary:' scripts/install.sh
assert_contains 'Universal MCP entry (works in any MCP client):' scripts/install.sh
assert_contains 'Quick terminal check:' scripts/install.sh
assert_contains 'echo "  args: ${SERVER_ARGS_TEXT}"' scripts/install.sh

echo "Canonical sister guardrails passed."
