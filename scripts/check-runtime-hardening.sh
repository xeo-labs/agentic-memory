#!/usr/bin/env bash
set -euo pipefail

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

assert_contains() {
  local pattern="$1"
  local file="$2"
  if command -v rg >/dev/null 2>&1; then
    rg -nF "$pattern" "$file" >/dev/null || fail "Missing required pattern in output: $pattern"
  else
    grep -n -F -- "$pattern" "$file" >/dev/null || fail "Missing required pattern in output: $pattern"
  fi
}

run_profile_check() {
  local profile="$1"
  local tmp
  tmp="$(mktemp)"

  local install_dir
  install_dir="$(mktemp -d)"

  if [ "$profile" = "server" ]; then
    AGENTIC_TOKEN="test-token" AGENTRA_INSTALL_DIR="$install_dir" ./scripts/install.sh --dry-run --profile="$profile" >"$tmp" 2>&1 || {
      cat "$tmp" >&2
      fail "Install dry-run failed for profile: $profile"
    }
  else
    AGENTRA_INSTALL_DIR="$install_dir" ./scripts/install.sh --dry-run --profile="$profile" >"$tmp" 2>&1 || {
      cat "$tmp" >&2
      fail "Install dry-run failed for profile: $profile"
    }
  fi

  assert_contains "Install profile: $profile" "$tmp"
  assert_contains "100% Install complete" "$tmp"
  assert_contains "Install complete:" "$tmp"
  assert_contains "Optional feedback:" "$tmp"

  if [ "$profile" = "server" ]; then
    assert_contains "Quick terminal checks:" "$tmp"
    assert_contains "Generate a token" "$tmp"
    assert_contains "AGENTIC_TOKEN" "$tmp"
    assert_contains "Start MCP with auth, connect clients, then restart clients." "$tmp"
  elif [ "$profile" = "terminal" ]; then
    assert_contains "MCP client summary:" "$tmp"
    assert_contains "Universal MCP entry (works in any MCP client):" "$tmp"
    assert_contains "Quick terminal check:" "$tmp"
    assert_contains "Restart your MCP client/system so it reloads MCP config." "$tmp"
  else
    assert_contains "MCP client summary:" "$tmp"
    assert_contains "Universal MCP entry (works in any MCP client):" "$tmp"
    assert_contains "Quick terminal check:" "$tmp"
    assert_contains "Restart any configured MCP client" "$tmp"
  fi

  rm -f "$tmp"
  rm -rf "$install_dir"
}

run_profile_check desktop
run_profile_check terminal
run_profile_check server

echo "Runtime hardening guardrails passed."
