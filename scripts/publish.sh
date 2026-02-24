#!/bin/bash
set -e

DO_PUBLISH=false
if [ "${1:-}" = "--publish" ]; then
  DO_PUBLISH=true
fi

CORE_VERSION="$(grep -m1 '^version\\s*=\\s*"' crates/agentic-memory/Cargo.toml | sed -E 's/.*"([^"]+)".*/\\1/')"
MCP_VERSION="$(grep -m1 '^version\\s*=\\s*"' crates/agentic-memory-mcp/Cargo.toml | sed -E 's/.*"([^"]+)".*/\\1/')"
NOTE_DIR="release-notes"
NOTE_FILE="${NOTE_DIR}/v${CORE_VERSION}.md"

ensure_release_note() {
  mkdir -p "${NOTE_DIR}"
  if [ ! -f "${NOTE_FILE}" ]; then
    cat > "${NOTE_FILE}" <<EOF
## TEMPLATE_DRAFT: REPLACE BEFORE PUBLISH

## Executive Summary

AgenticMemory v${CORE_VERSION} (with MCP v${MCP_VERSION}) delivers production-grade runtime improvements with clearer operator controls and measurable deployment value.

## Business Impact

This release lowers operational friction for teams adopting persistent agent memory, improves reliability across session boundaries, and reduces repeated manual troubleshooting effort.

## Rollout Guidance

Publish core first, validate availability on crates.io, then publish MCP and verify MCP client registration plus artifact sync in staging before broad rollout.

## Source Links

- https://github.com/agentralabs/agentic-memory/compare/v${CORE_VERSION}...HEAD
EOF
    echo "Release note template created at ${NOTE_FILE}."
    echo "Publish gate blocked until you replace template text with final business notes."
    exit 1
  fi

  python3 - <<'PY' "${NOTE_FILE}"
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
required = [
    "## Executive Summary",
    "## Business Impact",
    "## Rollout Guidance",
    "## Source Links",
]
for heading in required:
    if heading not in text:
        print(f"Missing required heading: {heading}")
        sys.exit(1)

if "template_draft" in text.lower():
    print("Template marker still present in release notes.")
    sys.exit(1)

if "as an ai" in text.lower():
    print("Release notes contain forbidden phrasing: as an ai")
    sys.exit(1)

paragraphs = []
for block in re.split(r"\n\s*\n", text):
    b = block.strip()
    if not b or b.startswith("##") or b.startswith("- "):
        continue
    paragraphs.append(b)

if len(paragraphs) < 3:
    print("Release note must contain at least 3 narrative paragraphs.")
    sys.exit(1)

for idx, p in enumerate(paragraphs[:3], start=1):
    if len(p) < 120:
        print(f"Paragraph {idx} is too short ({len(p)} chars).")
        sys.exit(1)
PY
}

ensure_release_note

echo "=== Publishing AgenticMemory paired crates to crates.io ==="

# Verify logged in
cargo login --help > /dev/null

# Run all tests
cargo test --workspace

# Check formatting
cargo fmt --check

# Check clippy
cargo clippy --workspace -- -D warnings

# Dry run publish (paired crates: core first, then MCP)
echo "Dry run: agentic-memory"
cd crates/agentic-memory
cargo publish --dry-run
cd ../..

echo "Preflight: agentic-memory-mcp (build + lint path)"
cargo check -p agentic-memory-mcp
echo "Note: skipping MCP crates.io dry-run until the new core crate version is published."

echo ""
if [ "${DO_PUBLISH}" = true ]; then
  echo "Publishing core crate..."
  (cd crates/agentic-memory && cargo publish)
  echo "Waiting for crates.io propagation..."
  sleep 45
  echo "Publishing MCP crate..."
  (cd crates/agentic-memory-mcp && cargo publish)

  if ! command -v gh >/dev/null 2>&1; then
    echo "Error: gh CLI is required to create GitHub release notes."
    exit 1
  fi

  echo "Creating GitHub release..."
  gh release create "v${CORE_VERSION}" \
    --title "AgenticMemory v${CORE_VERSION}" \
    --notes-file "${NOTE_FILE}" \
    --target "$(git rev-parse HEAD)"
  echo "Publish + release complete."
else
  echo "Dry run successful. To actually publish:"
  echo "  ./scripts/publish.sh --publish"
fi
