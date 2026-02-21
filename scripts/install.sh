#!/bin/bash
# AgenticMemory — one-liner install script
# Downloads pre-built binary and configures Claude Desktop/Code.
#
# Usage:
#   curl -fsSL https://agentralabs.tech/install/memory | bash
#
# Options:
#   --version=X.Y.Z   Pin a specific version (default: latest)
#   --dir=/path        Override install directory (default: ~/.local/bin)
#   --dry-run          Print actions without executing
#
# What it does:
#   1. Downloads agentic-memory-mcp binary to ~/.local/bin/
#   2. MERGES (not overwrites) MCP config into Claude Desktop and Claude Code
#   3. Leaves all existing MCP servers untouched
#
# Requirements: curl, jq

set -euo pipefail

# ── Constants ──────────────────────────────────────────────────────────
REPO="agentralabs/agentic-memory"
BINARY_NAME="agentic-memory-mcp"
SERVER_KEY="agentic-memory"
INSTALL_DIR="$HOME/.local/bin"
VERSION="latest"
DRY_RUN=false
CLAUDE_DESKTOP_CONFIGURED=false
CLAUDE_CODE_CONFIGURED=false

# ── Parse arguments ──────────────────────────────────────────────────
for arg in "$@"; do
    case "$arg" in
        --version=*) VERSION="${arg#*=}" ;;
        --dir=*)     INSTALL_DIR="${arg#*=}" ;;
        --dry-run)   DRY_RUN=true ;;
        --help|-h)
            echo "Usage: install.sh [--version=X.Y.Z] [--dir=/path] [--dry-run]"
            exit 0
            ;;
    esac
done

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
        x86_64|amd64)  arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
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

# ── Get latest release tag (empty when unavailable) ──────────────────
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
        | jq -r '.tag_name // empty' 2>/dev/null || true
}

# ── Download and extract binary ──────────────────────────────────────
download_binary() {
    local version="$1" platform="$2"
    local version_num="${version#v}"
    local asset_name="agentic-memory-${version_num}-${platform}.tar.gz"
    local url="https://github.com/${REPO}/releases/download/${version}/${asset_name}"

    echo "Downloading ${BINARY_NAME} ${version} (${platform})..."

    if [ "$DRY_RUN" = true ]; then
        echo "  [dry-run] Would download: ${url}"
        echo "  [dry-run] Would install to: ${INSTALL_DIR}/${BINARY_NAME}"
        return
    fi

    local tmpdir
    tmpdir="$(mktemp -d)"

    mkdir -p "$INSTALL_DIR"
    if ! curl -fsSL "$url" -o "${tmpdir}/${asset_name}"; then
        rm -rf "$tmpdir"
        return 1
    fi

    if ! tar xzf "${tmpdir}/${asset_name}" -C "$tmpdir"; then
        rm -rf "$tmpdir"
        return 1
    fi

    # Copy both binaries (amem CLI + MCP server)
    cp "${tmpdir}"/agentic-memory-*/amem "${INSTALL_DIR}/amem" 2>/dev/null || true
    cp "${tmpdir}"/agentic-memory-*/${BINARY_NAME} "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/amem" 2>/dev/null || true
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    rm -rf "$tmpdir"
    echo "  Installed to ${INSTALL_DIR}/${BINARY_NAME}"
}

# ── Source fallback when release artifacts are unavailable ────────────
install_from_source() {
    echo "Installing from source (cargo fallback)..."

    if ! command -v cargo &>/dev/null; then
        echo "Error: release artifacts are unavailable and cargo is not installed." >&2
        echo "Install Rust/Cargo first: https://rustup.rs" >&2
        exit 1
    fi

    local git_url="https://github.com/${REPO}.git"
    local cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
    local source_ref_text=""
    if [ -n "${VERSION:-}" ] && [ "${VERSION}" != "latest" ]; then
        source_ref_text="--tag ${VERSION} "
    fi

    if [ "$DRY_RUN" = true ]; then
        echo "  [dry-run] Would run: cargo install --git ${git_url} ${source_ref_text}--locked --force agentic-memory"
        echo "  [dry-run] Would run: cargo install --git ${git_url} ${source_ref_text}--locked --force agentic-memory-mcp"
        echo "  [dry-run] Would copy from ${cargo_bin}/(amem,${BINARY_NAME}) to ${INSTALL_DIR}/"
        return
    fi

    if [ -n "${VERSION:-}" ] && [ "${VERSION}" != "latest" ]; then
        cargo install --git "${git_url}" --tag "${VERSION}" --locked --force agentic-memory
        cargo install --git "${git_url}" --tag "${VERSION}" --locked --force agentic-memory-mcp
    else
        cargo install --git "${git_url}" --locked --force agentic-memory
        cargo install --git "${git_url}" --locked --force agentic-memory-mcp
    fi

    mkdir -p "${INSTALL_DIR}"
    cp "${cargo_bin}/amem" "${INSTALL_DIR}/amem"
    cp "${cargo_bin}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/amem" "${INSTALL_DIR}/${BINARY_NAME}"
    echo "  Installed from source to ${INSTALL_DIR}/amem and ${INSTALL_DIR}/${BINARY_NAME}"
}

# ── Merge MCP server into a config file ───────────────────────────────
# Uses jq to add our server WITHOUT touching other servers.
merge_config() {
    local config_file="$1"
    local config_dir
    config_dir="$(dirname "$config_file")"

    if [ "$DRY_RUN" = true ]; then
        echo "    [dry-run] Would merge into: ${config_file}"
        return
    fi

    mkdir -p "$config_dir"

    if [ -f "$config_file" ] && [ -s "$config_file" ]; then
        echo "    Existing config found, merging..."
        jq --arg key "$SERVER_KEY" \
           --arg cmd "${INSTALL_DIR}/${BINARY_NAME}" \
           '.mcpServers //= {} | .mcpServers[$key] = {"command": $cmd, "args": ["serve"]}' \
           "$config_file" > "$config_file.tmp" && mv "$config_file.tmp" "$config_file"
    else
        echo "    Creating new config..."
        jq -n --arg key "$SERVER_KEY" \
              --arg cmd "${INSTALL_DIR}/${BINARY_NAME}" \
           '{ "mcpServers": { ($key): { "command": $cmd, "args": ["serve"] } } }' \
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
    echo "  Done"
    CLAUDE_DESKTOP_CONFIGURED=true
}

# ── Configure Claude Code ────────────────────────────────────────────
configure_claude_code() {
    local config_file="$HOME/.claude/mcp.json"

    if [ -d "$HOME/.claude" ] || [ -f "$config_file" ]; then
        echo "  Claude Code..."
        merge_config "$config_file"
        echo "  Done"
        CLAUDE_CODE_CONFIGURED=true
    fi
}

print_client_help() {
    echo ""
    echo "MCP client summary:"
    if [ "$CLAUDE_DESKTOP_CONFIGURED" = true ]; then
        echo "  - Configured: Claude Desktop"
    fi
    if [ "$CLAUDE_CODE_CONFIGURED" = true ]; then
        echo "  - Configured: Claude Code"
    fi
    if [ "$CLAUDE_DESKTOP_CONFIGURED" = false ] && [ "$CLAUDE_CODE_CONFIGURED" = false ]; then
        echo "  - Claude Desktop/Code not detected (auto-config skipped)"
    fi
    echo ""
    echo "For Codex, Cursor, Windsurf, VS Code, Cline, or any MCP client, add:"
    echo "  command: ${INSTALL_DIR}/${BINARY_NAME}"
    echo "  args: [\"serve\"]"
    echo ""
    echo "Quick terminal check:"
    echo "  ${INSTALL_DIR}/${BINARY_NAME} serve"
    echo "  (Ctrl+C to stop after startup check)"
}

# ── Check PATH ────────────────────────────────────────────────────────
check_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo ""
        echo "Note: Add ${INSTALL_DIR} to your PATH if not already:"
        echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        echo ""
        echo "Add this line to your ~/.zshrc or ~/.bashrc to make it permanent."
    fi
}

# ── Main ──────────────────────────────────────────────────────────────
main() {
    echo "AgenticMemory Installer"
    echo "======================"
    echo ""

    check_deps

    local platform
    platform="$(detect_platform)"
    echo "Platform: ${platform}"

    local installed_from_release=false
    if [ "$VERSION" = "latest" ]; then
        VERSION="$(get_latest_version)"
    fi

    if [ -n "$VERSION" ] && [ "$VERSION" != "null" ]; then
        echo "Version: ${VERSION}"
        if download_binary "$VERSION" "$platform"; then
            installed_from_release=true
        else
            echo "Release artifact not found for ${VERSION}/${platform}; using source fallback."
        fi
    else
        echo "No GitHub release found; using source fallback."
    fi

    if [ "$installed_from_release" = false ]; then
        install_from_source
    fi

    echo ""
    echo "Configuring MCP clients..."
    configure_claude_desktop
    configure_claude_code
    print_client_help

    echo ""
    echo "Done! Memory defaults to ~/.brain.amem"
    echo "Restart any configured MCP client to activate."

    check_path
}

main "$@"
