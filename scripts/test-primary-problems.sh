#!/usr/bin/env bash
set -euo pipefail

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

assert_contains() {
  local text="$1"
  local pattern="$2"
  local label="$3"
  if command -v rg >/dev/null 2>&1; then
    printf '%s' "$text" | rg -q --fixed-strings "$pattern" || fail "${label}: missing '${pattern}'"
  else
    printf '%s' "$text" | grep -q -F -- "$pattern" || fail "${label}: missing '${pattern}'"
  fi
}

run_amem() {
  cargo run --quiet --bin amem -- "$@"
}

tmpdir="$(mktemp -d)"
brain="$tmpdir/primary.amem"
workspace="$tmpdir/workspace"
mkdir -p "$workspace"

echo "[1/8] Create memory artifact"
create_out="$(run_amem create "$brain")"
assert_contains "$create_out" "Created" "create"

echo "[2/8] Add longitudinal memory with confidence"
add_fact="$(run_amem add "$brain" fact "Release gate requires full regression evidence" --session 11 --confidence 0.93)"
add_decision="$(run_amem add "$brain" decision "Do not publish until edge-case suite is green" --session 11 --confidence 0.86)"
add_correction="$(run_amem add "$brain" correction "Publish only after stress plus smoke pass" --supersedes 1 --session 11 --confidence 0.95)"
assert_contains "$add_fact" "Added node" "add fact"
assert_contains "$add_decision" "Added node" "add decision"
assert_contains "$add_correction" "Added node" "add correction"

echo "[3/8] Validate retrieval controls (noise reduction)"
search_out="$(run_amem search "$brain" --event-types fact,decision,correction --min-confidence 0.8 --limit 10)"
hybrid_out="$(run_amem hybrid-search "$brain" "regression evidence" --text-weight 0.7 --vec-weight 0.3 --limit 10)"
assert_contains "$search_out" "results" "search"
assert_contains "$hybrid_out" "Node" "hybrid-search"

echo "[4/8] Validate contradiction + resolve path"
revise_out="$(run_amem revise "$brain" "Regression can be skipped" --threshold 0.55 --max-depth 4)"
resolve_out="$(run_amem resolve "$brain" 1)"
assert_contains "$revise_out" "Belief revision" "revise"
assert_contains "$resolve_out" "Current version" "resolve"

echo "[5/8] Validate quality and uncertainty reporting"
quality_out="$(run_amem quality "$brain" --low-confidence 0.45 --stale-decay 0.2 --limit 5)"
assert_contains "$quality_out" "Memory quality report" "quality"
assert_contains "$quality_out" "Status:" "quality"

echo "[6/8] Validate runtime handoff continuity"
runtime_sync_out="$(run_amem runtime-sync "$brain" --workspace "$workspace" --max-depth 2 --write-episode)"
sessions_out="$(run_amem sessions "$brain")"
assert_contains "$runtime_sync_out" "Wrote episode node" "runtime-sync"
assert_contains "$sessions_out" "Session" "sessions"

echo "[7/8] Validate long-horizon storage governance"
budget_out="$(run_amem budget "$brain" --horizon-years 20 --max-bytes 2147483648)"
assert_contains "$budget_out" "Projected size" "budget"
assert_contains "$budget_out" "Over budget:" "budget"

echo "[8/8] Validate focused regression tests"
cargo test --quiet -p agentic-memory --test phase5_quality test_memory_quality_detects_structural_and_confidence_issues
cargo test --quiet -p agentic-memory-mcp --test edge_cases test_memory_similar_with_query_text

echo "Primary memory problem checks passed (P01,P02,P05,P06,P07,P32,P33,P34,P42)"
