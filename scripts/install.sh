#!/bin/bash
# AgenticMemory — one-liner install script
# Downloads pre-built binary and configures Claude Desktop/Code.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/xeo-labs/agentic-memory/main/scripts/install.sh | bash
#
# Future short URL (when agentic.sh is set up):
#   curl -fsSL https://agentic.sh/memory | sh
#
# What it does:
#   1. Downloads agentic-memory-mcp binary to ~/.local/bin/
#   2. MERGES (not overwrites) MCP config into Claude Desktop and Claude Code
#   3. Leaves all existing MCP servers untouched
#
# Requirements: curl, jq

set -euo pipefail

# ── Constants ──────────────────────────────────────────────────────────
REPO="xeo-labs/agentic-memory"
BINARY_NAME="agentic-memory-mcp"
SERVER_KEY="agentic-memory"
INSTALL_DIR="$HOME/.local/bin"

# ── Detect platform ───────────────────────────────────────────────────
detect_platform() {
    local os arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$os" in
        darwin) os="darwin" ;;
        linux)  os="linux" ;;
        *)      echo "Error: Unsupported OS: $os" >&2; exit 1 ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch="x64" ;;
        arm64|aarch64) arch="arm64" ;;
        *)             echo "Error: Unsupported architecture: $arch" >&2; exit 1 ;;
    esac

    echo "${os}-${arch}"
}

# ── Check dependencies ────────────────────────────────────────────────
check_deps() {
    for cmd in curl jq; do
        if ! command -v "$cmd" &>/dev/null; then
            echo "Error: '$cmd' is required but not installed." >&2
            if [ "$cmd" = "jq" ]; then
                echo "  Install: brew install jq  (macOS) or apt install jq (Linux)" >&2
            fi
            exit 1
        fi
    done
}

# ── Get latest release tag ────────────────────────────────────────────
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | jq -r '.tag_name'
}

# ── Download binary ───────────────────────────────────────────────────
download_binary() {
    local version="$1" platform="$2"
    local asset_name="${BINARY_NAME}-${platform}"
    local url="https://github.com/${REPO}/releases/download/${version}/${asset_name}"

    echo "Downloading ${BINARY_NAME} ${version} (${platform})..."
    mkdir -p "$INSTALL_DIR"
    curl -fsSL "$url" -o "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    echo "  Installed to ${INSTALL_DIR}/${BINARY_NAME}"
}

# ── Merge MCP server into a config file ───────────────────────────────
# Uses jq to add our server WITHOUT touching other servers.
merge_config() {
    local config_file="$1"
    local config_dir
    config_dir="$(dirname "$config_file")"

    # Ensure directory exists
    mkdir -p "$config_dir"

    if [ -f "$config_file" ] && [ -s "$config_file" ]; then
        # Config exists — merge our server in, preserve everything else
        echo "    Existing config found, merging..."
        jq --arg key "$SERVER_KEY" \
           --arg cmd "${INSTALL_DIR}/${BINARY_NAME}" \
           '.mcpServers //= {} | .mcpServers[$key] = {"command": $cmd, "args": ["serve"]}' \
           "$config_file" > "$config_file.tmp" && mv "$config_file.tmp" "$config_file"
    else
        # No config — create fresh with only our server
        echo "    Creating new config..."
        jq -n --arg cmd "${INSTALL_DIR}/${BINARY_NAME}" \
           '{ "mcpServers": { "agentic-memory": { "command": $cmd, "args": ["serve"] } } }' \
           > "$config_file"
    fi
}

# ── Configure Claude Desktop ─────────────────────────────────────────
configure_claude_desktop() {
    local config_file
    case "$(uname -s)" in
        Darwin) config_file="$HOME/Library/Application Support/Claude/claude_desktop_config.json" ;;
        Linux)  config_file="${XDG_CONFIG_HOME:-$HOME/.config}/Claude/claude_desktop_config.json" ;;
        *)      return ;;
    esac

    echo "  Claude Desktop..."
    merge_config "$config_file"
    echo "  ✅ Claude Desktop configured"
}

# ── Configure Claude Code ────────────────────────────────────────────
configure_claude_code() {
    local config_file="$HOME/.claude/mcp.json"

    # Only configure if Claude Code directory exists
    if [ -d "$HOME/.claude" ] || [ -f "$config_file" ]; then
        echo "  Claude Code..."
        merge_config "$config_file"
        echo "  ✅ Claude Code configured"
    fi
}

# ── Check PATH ────────────────────────────────────────────────────────
check_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo ""
        echo "Note: Add ~/.local/bin to your PATH if not already:"
        echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        echo ""
        echo "Add this line to your ~/.zshrc or ~/.bashrc to make it permanent."
    fi
}

# ── Main ──────────────────────────────────────────────────────────────
main() {
    echo "AgenticMemory Installer"
    echo "━━━━━━━━━━━━━━━━━━━━━━"
    echo ""

    check_deps

    local platform
    platform="$(detect_platform)"

    local version
    version="$(get_latest_version)"
    if [ -z "$version" ] || [ "$version" = "null" ]; then
        echo "Error: Could not determine latest release version." >&2
        echo "  You can install from source: cargo install agentic-memory-mcp" >&2
        exit 1
    fi

    download_binary "$version" "$platform"

    echo ""
    echo "Configuring MCP clients..."
    configure_claude_desktop
    configure_claude_code

    echo ""
    echo "Done! Memory defaults to ~/.brain.amem"
    echo "Restart Claude Desktop / Claude Code to activate."

    check_path
}

main "$@"
