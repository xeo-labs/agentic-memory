#!/usr/bin/env bash
# check-canonical-sister.sh — Universal canonical sister guardrail
#
# This script enforces structural parity across all Agentra sister repos.
# The assertion body is IDENTICAL across all sisters. Only the header
# variables differ. If any check fails, the sister is NOT release-ready.
#
set -euo pipefail

# ── Sister-specific configuration (ONLY section that differs per sister) ─────
SISTER_KEY="memory"
SISTER_NAME="AgenticMemory"
FRONTMATTER_EXTRA=(
  "docs/public/file-format.md"
  "docs/public/rust-api.md"
)
# ── End sister-specific configuration ────────────────────────────────────────

# ── Shared helpers ───────────────────────────────────────────────────────────

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

assert_dir() {
  [ -d "$1" ] || fail "Missing required directory: $1"
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

assert_frontmatter_status_stable() {
  local file="$1"
  assert_file "$file"
  local status
  status="$(awk '
    NR==1 && $0=="---" {fm=1; next}
    fm && /^status:[[:space:]]*/ {
      sub(/^status:[[:space:]]*/, "", $0)
      gsub(/[[:space:]]+/, "", $0)
      print tolower($0)
      exit
    }
    fm && $0=="---" {exit}
  ' "$file")"
  [ "$status" = "stable" ] || fail "Public doc must include frontmatter status: stable ($file)"
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

# ── 1. Core file existence ──────────────────────────────────────────────────

assert_file "docs/ecosystem/CANONICAL_SISTER_KIT.md"
assert_file "scripts/install.sh"
assert_file "scripts/check-install-commands.sh"
assert_file "scripts/check-runtime-hardening.sh"
assert_file "scripts/test-primary-problems.sh"
assert_file "docs/quickstart.md"
assert_file "docs/concepts.md"
assert_file "docs/integration-guide.md"
assert_file "docs/faq.md"
assert_file "docs/benchmarks.md"
assert_file "docs/api-reference.md"
assert_one_of "docs/file-format.md" "docs/LIMITATIONS.md"
assert_file "docs/public/primary-problem-coverage.md"
assert_file "docs/public/initial-problem-coverage.md"
assert_file "docs/public/sister.manifest.json"

# ── 2. Asset existence ──────────────────────────────────────────────────────

assert_dir "assets"
assert_file "assets/github-hero-pane.svg"
assert_file "assets/github-terminal-pane.svg"

# ── 3. Git tracking validation ──────────────────────────────────────────────

assert_not_tracked "ECOSYSTEM-CONVENTIONS.md"
assert_no_tracked_prefix "docs/internal/*"

# ── 4. CANONICAL_SISTER_KIT.md section headers ──────────────────────────────

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
assert_contains '## 12. Web Docs Grouping Contract' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '## 13. Runtime Isolation and Universal MCP Hardening (Mandatory)' docs/ecosystem/CANONICAL_SISTER_KIT.md

# ── 5. CANONICAL_SISTER_KIT.md in public docs mirror ────────────────────────

assert_contains '## 13. Runtime Isolation and Universal MCP Hardening (Mandatory)' docs/public/ecosystem/CANONICAL_SISTER_KIT.md
if [ -f planning-docs/CANONICAL_SISTER_KIT.md ]; then
  assert_contains '## 13. Runtime Isolation and Universal MCP Hardening (Mandatory)' planning-docs/CANONICAL_SISTER_KIT.md
fi

# ── 6. Section 13 runtime isolation key phrases ─────────────────────────────

assert_contains 'No silent fallback behavior for invalid enum/mode/depth/type parameters.' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'Deterministic per-project identity is required (canonical-path hashing or equivalent).' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'Do not bind to unrelated "latest cached" project state.' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'Stale/dead lock recovery is mandatory.' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'Support `desktop`, `terminal`, and `server` profiles.' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'Post-install output must include restart guidance and optional feedback guidance.' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'Server profile/runtime must enforce token-based auth gate (`AGENTIC_TOKEN` or token file equivalent).' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'Release gate requires automated stress/regression proof for:' docs/ecosystem/CANONICAL_SISTER_KIT.md

# ── 7. Ecosystem URLs and key requirement phrases ───────────────────────────

assert_contains 'https://agentralabs.tech/docs/ecosystem-feature-reference' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'https://agentralabs.tech/docs/sister-docs-catalog' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'docs folder required for every sister' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'web docs wiring is mandatory before release' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'Installer strength/completeness is mandatory for every new sister.' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'Before implementing a new sister installer, review `agentic-memory/scripts/install.sh`, `agentic-vision/scripts/install.sh`, and `agentic-codebase/scripts/install.sh` as benchmark baselines.' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains '`agentic-memory-mcp`, `agentic-vision-mcp`, `agentic-codebase-mcp`, and `agentic-identity-mcp` are treated as live ecosystem infrastructure.' docs/ecosystem/CANONICAL_SISTER_KIT.md
assert_contains 'New sister planning, implementation, and validation must explicitly use those MCP servers where applicable (design support, integration checks, stress/regression checks).' docs/ecosystem/CANONICAL_SISTER_KIT.md

# ── 8. Sister manifest validation (parameterized) ───────────────────────────

assert_contains "\"key\": \"${SISTER_KEY}\"" docs/public/sister.manifest.json
assert_contains "\"name\": \"${SISTER_NAME}\"" docs/public/sister.manifest.json
assert_contains '"page_ids": [' docs/public/sister.manifest.json

# ── 9. Public docs frontmatter (common baseline) ────────────────────────────

FRONTMATTER_BASELINE=(
  "docs/public/experience-with-vs-without.md"
  "docs/public/quickstart.md"
  "docs/public/installation.md"
  "docs/public/command-surface.md"
  "docs/public/runtime-install-sync.md"
  "docs/public/integration-guide.md"
  "docs/public/concepts.md"
  "docs/public/api-reference.md"
  "docs/public/benchmarks.md"
  "docs/public/faq.md"
)

for doc in "${FRONTMATTER_BASELINE[@]}"; do
  assert_frontmatter_status_stable "$doc"
done

for doc in "${FRONTMATTER_EXTRA[@]}"; do
  assert_frontmatter_status_stable "$doc"
done

# ── 10. README canonical layout ─────────────────────────────────────────────

assert_contains '<img src="assets/github-hero-pane.svg"' README.md
assert_contains '<img src="assets/github-terminal-pane.svg"' README.md
assert_contains '## Install' README.md
assert_contains '## Quickstart' README.md
assert_contains '## How It Works' README.md
assert_image_spacing

# ── 11. Install script canonical output ─────────────────────────────────────

assert_contains 'MCP client summary:' scripts/install.sh
assert_contains 'Universal MCP entry (works in any MCP client):' scripts/install.sh
assert_contains 'Quick terminal check:' scripts/install.sh
assert_contains 'echo "  args: ${SERVER_ARGS_TEXT}"' scripts/install.sh
assert_contains 'After restart, confirm' scripts/install.sh
assert_contains 'Optional feedback:' scripts/install.sh
assert_contains 'AGENTIC_TOKEN' scripts/install.sh

# ── 12. CI workflow presence ────────────────────────────────────────────────

assert_file ".github/workflows/ci.yml"
assert_file ".github/workflows/release.yml"
assert_file ".github/workflows/canonical-sister-guardrails.yml"
assert_file ".github/workflows/install-command-guardrails.yml"

# ── Done ────────────────────────────────────────────────────────────────────

echo "Canonical sister guardrails passed."
