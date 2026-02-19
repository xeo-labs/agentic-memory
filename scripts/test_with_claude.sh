#!/bin/bash
set -e

echo "=== AgenticMemory + Claude Code Integration Test ==="

# Build release binary
cargo build --release

# Memory file defaults to ~/.brain.amem (zero-config)
TEST_MEMORY="$HOME/.brain.amem"
BINARY="$(pwd)/target/release/agentic-memory-mcp"

# Configure Claude Desktop (macOS example)
CONFIG_FILE="$HOME/Library/Application Support/Claude/claude_desktop_config.json"

# Check for jq
if ! command -v jq &>/dev/null; then
    echo "Error: jq is required. Install with: brew install jq"
    exit 1
fi

# Backup existing config
if [ -f "$CONFIG_FILE" ]; then
    cp "$CONFIG_FILE" "$CONFIG_FILE.bak"
    echo "Backed up existing config"
fi

# MERGE our server into existing config (not overwrite)
if [ -f "$CONFIG_FILE" ] && [ -s "$CONFIG_FILE" ]; then
    echo "Merging into existing config..."
    jq --arg cmd "$BINARY" \
       '.mcpServers //= {} | .mcpServers["agentic-memory"] = {"command": $cmd, "args": ["serve"]}' \
       "$CONFIG_FILE" > "$CONFIG_FILE.tmp" && mv "$CONFIG_FILE.tmp" "$CONFIG_FILE"
else
    echo "Creating new config..."
    mkdir -p "$(dirname "$CONFIG_FILE")"
    jq -n --arg cmd "$BINARY" \
       '{ "mcpServers": { "agentic-memory": { "command": $cmd, "args": ["serve"] } } }' \
       > "$CONFIG_FILE"
fi

echo "✅ Config written to: $CONFIG_FILE"
echo "   Memory file: $TEST_MEMORY (default)"
echo ""
echo "Now:"
echo "1. Restart Claude Desktop"
echo "2. Ask Claude to use the memory tools"
echo "3. Example prompts:"
echo "   - 'Remember that I prefer Rust over Python'"
echo "   - 'What do you remember about my preferences?'"
echo "   - 'Why did you recommend X last time?'"
echo ""
echo "Press Enter when done testing..."
read

# Verify memory was created
if [ -f "$TEST_MEMORY" ]; then
    echo "✓ Memory file created"
    ls -la "$TEST_MEMORY"
else
    echo "✗ Memory file not created - test failed"
    exit 1
fi

# Restore config
if [ -f "$CONFIG_FILE.bak" ]; then
    mv "$CONFIG_FILE.bak" "$CONFIG_FILE"
    echo "✓ Original config restored"
fi

echo "=== Test Complete ==="
