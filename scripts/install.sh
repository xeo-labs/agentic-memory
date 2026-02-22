#!/bin/bash
# AgenticMemory — one-liner install script
# Downloads pre-built binary and auto-configures detected MCP clients.
#
# Usage:
#   curl -fsSL https://agentralabs.tech/install/memory | bash
#
# Options:
#   --version=X.Y.Z   Pin a specific version (default: latest)
#   --dir=/path        Override install directory (default: ~/.local/bin)
#   --profile=<name>   Install profile: desktop | terminal | server (default: desktop)
#   --dry-run          Print actions without executing
#
# What it does:
#   1. Downloads agentic-memory-mcp binary to ~/.local/bin/
#   2. MERGES (not overwrites) MCP config into detected MCP client configs
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
PROFILE="${AGENTRA_INSTALL_PROFILE:-desktop}"
DRY_RUN=false
BAR_ONLY="${AGENTRA_INSTALL_BAR_ONLY:-1}"
MCP_ENTRYPOINT=""
SERVER_ARGS_JSON='["serve"]'
SERVER_ARGS_TEXT='["serve"]'
SERVER_CHECK_CMD_SUFFIX=" serve"
MCP_CONFIGURED_CLIENTS=()
MCP_SCANNED_CONFIG_FILES=()

# ── Parse arguments ──────────────────────────────────────────────────
for arg in "$@"; do
    case "$arg" in
        --version=*) VERSION="${arg#*=}" ;;
        --dir=*)     INSTALL_DIR="${arg#*=}" ;;
        --profile=*) PROFILE="${arg#*=}" ;;
        --dry-run)   DRY_RUN=true ;;
        --help|-h)
            echo "Usage: install.sh [--version=X.Y.Z] [--dir=/path] [--profile=desktop|terminal|server] [--dry-run]"
            exit 0
            ;;
    esac
done

MCP_ENTRYPOINT="${INSTALL_DIR}/${BINARY_NAME}-agentra"

# ── Progress output (bar-only mode by default) ───────────────────────
exec 3>&1
if [ "$BAR_ONLY" = "1" ] && [ "$DRY_RUN" = false ]; then
    exec 1>/dev/null
fi

PROGRESS=0
BAR_WIDTH=36

draw_progress() {
    local percent="$1"
    local label="$2"
    local filled=$((percent * BAR_WIDTH / 100))
    local empty=$((BAR_WIDTH - filled))
    printf "\r[" >&3
    printf "%${filled}s" "" | tr " " "#" >&3
    printf "%${empty}s" "" | tr " " "-" >&3
    printf "] %3d%% %s" "$percent" "$label" >&3
}

set_progress() {
    local percent="$1"
    local label="$2"
    PROGRESS="$percent"
    draw_progress "$percent" "$label"
}

finish_progress() {
    printf "\n" >&3
}

run_with_progress() {
    local start="$1"
    local end="$2"
    local label="$3"
    shift 3

    local log_file
    log_file="$(mktemp)"
    local current="$start"

    set_progress "$current" "$label"
    "$@" >"$log_file" 2>&1 &
    local cmd_pid=$!

    while kill -0 "$cmd_pid" 2>/dev/null; do
        if [ "$current" -lt $((end - 1)) ]; then
            current=$((current + 1))
            set_progress "$current" "$label"
        fi
        sleep 0.2
    done

    if ! wait "$cmd_pid"; then
        finish_progress
        echo "Install failed during: ${label}" >&3
        tail -n 80 "$log_file" >&3 || true
        rm -f "$log_file"
        return 1
    fi

    rm -f "$log_file"
    set_progress "$end" "$label"
}

validate_profile() {
    case "$PROFILE" in
        desktop|terminal|server) ;;
        *)
            echo "Error: invalid profile '${PROFILE}'. Use desktop, terminal, or server." >&2
            exit 1
            ;;
    esac
}

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
    if ! curl -fsSL "$url" -o "${tmpdir}/${asset_name}" 2>/dev/null; then
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
        run_with_progress 45 68 "Installing agentic-memory" \
            cargo install --git "${git_url}" --tag "${VERSION}" --locked --force agentic-memory
        run_with_progress 68 85 "Installing agentic-memory-mcp" \
            cargo install --git "${git_url}" --tag "${VERSION}" --locked --force agentic-memory-mcp
    else
        run_with_progress 45 68 "Installing agentic-memory" \
            cargo install --git "${git_url}" --locked --force agentic-memory
        run_with_progress 68 85 "Installing agentic-memory-mcp" \
            cargo install --git "${git_url}" --locked --force agentic-memory-mcp
    fi

    mkdir -p "${INSTALL_DIR}"
    cp "${cargo_bin}/amem" "${INSTALL_DIR}/amem"
    cp "${cargo_bin}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/amem" "${INSTALL_DIR}/${BINARY_NAME}"
    echo "  Installed from source to ${INSTALL_DIR}/amem and ${INSTALL_DIR}/${BINARY_NAME}"
}

install_mcp_entrypoint() {
    if [ "$DRY_RUN" = true ]; then
        echo "  [dry-run] Would install MCP launcher to: ${MCP_ENTRYPOINT}"
        return
    fi

    mkdir -p "${INSTALL_DIR}"
    cat >"${MCP_ENTRYPOINT}" <<EOF
#!/usr/bin/env bash
set -eo pipefail

BIN="${INSTALL_DIR}/${BINARY_NAME}"

find_memory() {
    local candidate
    local found=""

    for candidate in \
        "\${AGENTRA_AMEM_PATH:-}" \
        "\${AGENTRA_MEMORY_PATH:-}" \
        "\$HOME/.brain.amem" \
        "\$HOME/.agentra/memory/default.amem" \
        "\$PWD/.brain.amem" \
        "\$PWD/memory.amem"; do
        if [ -n "\$candidate" ] && [ -f "\$candidate" ]; then
            found="\$candidate"
            break
        fi
    done

    [ -n "\$found" ] && printf '%s' "\$found"
}

args=("\$@")
has_memory=0
has_command=0

for arg in "\${args[@]}"; do
    case "\$arg" in
        -h|--help|-V|--version) has_command=1 ;;
        -m|--memory|--memory=*) has_memory=1 ;;
        serve|validate|info|delete|export|compact|stats|help) has_command=1 ;;
    esac
done

if [ "\$has_memory" -eq 0 ]; then
    memory_path="\$(find_memory || true)"
    if [ -n "\$memory_path" ]; then
        args=(--memory "\$memory_path" "\${args[@]}")
    fi
fi

if [ "\$has_command" -eq 0 ]; then
    args+=("serve")
fi

exec "\$BIN" "\${args[@]}"
EOF
    chmod +x "${MCP_ENTRYPOINT}"
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
           --arg cmd "${MCP_ENTRYPOINT}" \
           --argjson args "$SERVER_ARGS_JSON" \
           '.mcpServers //= {} | .mcpServers[$key] = {"command": $cmd, "args": $args}' \
           "$config_file" > "$config_file.tmp" && mv "$config_file.tmp" "$config_file"
    else
        echo "    Creating new config..."
        jq -n --arg key "$SERVER_KEY" \
              --arg cmd "${MCP_ENTRYPOINT}" \
              --argjson args "$SERVER_ARGS_JSON" \
           '{ "mcpServers": { ($key): { "command": $cmd, "args": $args } } }' \
           > "$config_file"
    fi
}

upsert_codex_config_block() {
    local codex_config="$1"
    local tmp_file
    tmp_file="$(mktemp)"

    if [ -f "$codex_config" ]; then
        awk -v section="[mcp_servers.${SERVER_KEY}]" '
            BEGIN { skip = 0 }
            $0 == section { skip = 1; next }
            skip && /^\[.*\]$/ { skip = 0 }
            !skip { print }
        ' "$codex_config" > "$tmp_file"
    else
        : > "$tmp_file"
    fi

    {
        echo ""
        echo "[mcp_servers.${SERVER_KEY}]"
        echo "command = \"${MCP_ENTRYPOINT}\""
        echo "args = ${SERVER_ARGS_JSON}"
    } >> "$tmp_file"

    mv "$tmp_file" "$codex_config"
}

record_mcp_client() {
    local client_name="$1"
    MCP_CONFIGURED_CLIENTS+=("$client_name")
}

record_mcp_config_path() {
    local config_file="$1"
    MCP_SCANNED_CONFIG_FILES+=("$config_file")
}

is_known_mcp_config_path() {
    local config_file="$1"
    local known
    for known in "${MCP_SCANNED_CONFIG_FILES[@]}"; do
        if [ "$known" = "$config_file" ]; then
            return 0
        fi
    done
    return 1
}

configure_json_client_if_present() {
    local client_name="$1"
    local config_file="$2"
    local detect_path="${3:-$(dirname "$config_file")}"

    if [ -f "$config_file" ] || [ -d "$detect_path" ]; then
        echo "  ${client_name}..."
        merge_config "$config_file"
        echo "  Done"
        record_mcp_client "$client_name"
        record_mcp_config_path "$config_file"
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
    record_mcp_client "Claude Desktop"
    record_mcp_config_path "$config_file"
}

# ── Configure Claude Code ────────────────────────────────────────────
configure_claude_code() {
    local config_file="$HOME/.claude/mcp.json"

    if [ -d "$HOME/.claude" ] || [ -f "$config_file" ]; then
        echo "  Claude Code..."
        merge_config "$config_file"
        echo "  Done"
        record_mcp_client "Claude Code"
        record_mcp_config_path "$config_file"
    fi
}

configure_codex() {
    local codex_home="${CODEX_HOME:-$HOME/.codex}"
    local codex_config="${codex_home}/config.toml"
    local codex_cmd_args=("${MCP_ENTRYPOINT}" "serve")

    if ! command -v codex >/dev/null 2>&1 && [ ! -d "$codex_home" ] && [ ! -f "$codex_config" ]; then
        return
    fi

    echo "  Codex..."
    if [ "$DRY_RUN" = true ]; then
        echo "    [dry-run] Would run: codex mcp add ${SERVER_KEY} -- ${codex_cmd_args[*]}"
    elif command -v codex >/dev/null 2>&1; then
        codex mcp remove "$SERVER_KEY" >/dev/null 2>&1 || true
        if ! codex mcp add "$SERVER_KEY" -- "${codex_cmd_args[@]}" >/dev/null 2>&1; then
            echo "    Warning: could not auto-configure Codex via CLI."
            echo "    Run: codex mcp add ${SERVER_KEY} -- ${codex_cmd_args[*]}"
            return
        fi
    else
        mkdir -p "$codex_home"
        if [ ! -f "$codex_config" ]; then
            touch "$codex_config"
        fi
        upsert_codex_config_block "$codex_config"
    fi
    echo "  Done"
    record_mcp_client "Codex"
    record_mcp_config_path "$codex_config"
}

configure_generic_mcp_json_files() {
    local root
    local file
    local roots=(
        "$HOME/.config"
        "$HOME/Library/Application Support"
        "$HOME/.cursor"
        "$HOME/.windsurf"
        "$HOME/.codeium"
        "$HOME/.claude"
    )

    for root in "${roots[@]}"; do
        [ -d "$root" ] || continue
        while IFS= read -r file; do
            [ -n "$file" ] || continue
            if is_known_mcp_config_path "$file"; then
                continue
            fi
            echo "  Generic MCP config (${file})..."
            merge_config "$file"
            echo "  Done"
            record_mcp_client "Generic MCP JSON"
            record_mcp_config_path "$file"
        done < <(find "$root" -maxdepth 6 -type f \
            \( -name "mcp.json" -o -name "mcp_config.json" -o -name "claude_desktop_config.json" -o -name "cline_mcp_settings.json" \) \
            2>/dev/null | sort -u)
    done
}

configure_mcp_clients() {
    configure_claude_desktop
    configure_claude_code
    configure_json_client_if_present "Cursor" "$HOME/.cursor/mcp.json" "$HOME/.cursor"
    configure_json_client_if_present "Windsurf" "$HOME/.windsurf/mcp.json" "$HOME/.windsurf"
    configure_json_client_if_present "Windsurf (Codeium)" "$HOME/.codeium/windsurf/mcp_config.json" "$HOME/.codeium/windsurf"
    if [ "$(uname -s)" = "Darwin" ]; then
        configure_json_client_if_present "VS Code" "$HOME/Library/Application Support/Code/User/mcp.json" "$HOME/Library/Application Support/Code/User"
        configure_json_client_if_present "VS Code + Cline" "$HOME/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json" "$HOME/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev"
        configure_json_client_if_present "VSCodium" "$HOME/Library/Application Support/VSCodium/User/mcp.json" "$HOME/Library/Application Support/VSCodium/User"
    else
        configure_json_client_if_present "VS Code" "${XDG_CONFIG_HOME:-$HOME/.config}/Code/User/mcp.json" "${XDG_CONFIG_HOME:-$HOME/.config}/Code/User"
        configure_json_client_if_present "VS Code + Cline" "${XDG_CONFIG_HOME:-$HOME/.config}/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json" "${XDG_CONFIG_HOME:-$HOME/.config}/Code/User/globalStorage/saoudrizwan.claude-dev"
        configure_json_client_if_present "VSCodium" "${XDG_CONFIG_HOME:-$HOME/.config}/VSCodium/User/mcp.json" "${XDG_CONFIG_HOME:-$HOME/.config}/VSCodium/User"
    fi
    configure_codex
    configure_generic_mcp_json_files
}

print_client_help() {
    local client
    local configured_count="${#MCP_CONFIGURED_CLIENTS[@]}"

    echo ""
    echo "MCP client summary:"
    if [ "$configured_count" -gt 0 ]; then
        for client in "${MCP_CONFIGURED_CLIENTS[@]}"; do
            echo "  - Configured: ${client}"
        done
    else
        echo "  - No known MCP client config detected (auto-config skipped)"
    fi
    echo ""
    echo "Universal MCP entry (works in any MCP client):"
    echo "  command: ${MCP_ENTRYPOINT}"
    echo "  args: ${SERVER_ARGS_TEXT}"
    echo ""
    echo "Quick terminal check:"
    echo "  ${INSTALL_DIR}/${BINARY_NAME}${SERVER_CHECK_CMD_SUFFIX}"
    echo "  (Ctrl+C to stop after startup check)"
}

print_profile_help() {
    echo ""
    echo "Install profile: ${PROFILE}"
    case "$PROFILE" in
        desktop)
            echo "  - Binary installed"
            echo "  - Detected MCP client configs merged (Claude/Codex/Cursor/Windsurf/VS Code/etc.)"
            ;;
        terminal)
            echo "  - Binary installed"
            echo "  - Detected MCP client configs merged (same as desktop profile)"
            echo "  - Native terminal usage remains available"
            ;;
        server)
            echo "  - Binary installed"
            echo "  - No desktop config files were changed"
            echo "  - Suitable for remote/server hosts"
            echo "  - Server deployments should enforce auth (token/reverse-proxy/TLS)"
            ;;
    esac
}

print_terminal_server_help() {
    echo ""
    echo "Manual MCP config for any client:"
    echo "  command: ${MCP_ENTRYPOINT}"
    echo "  args: ${SERVER_ARGS_TEXT}"
    echo ""
    echo "Server authentication setup:"
    echo "  TOKEN=\$(openssl rand -hex 32)"
    echo "  export AGENTIC_TOKEN=\"\$TOKEN\""
    echo "  # Clients must send: Authorization: Bearer \$TOKEN"
    echo ""
    echo "Quick terminal checks:"
    echo "  ${INSTALL_DIR}/amem --help"
    echo "  ${INSTALL_DIR}/${BINARY_NAME}${SERVER_CHECK_CMD_SUFFIX}"
    echo "  (Ctrl+C to stop after startup check)"
}

print_post_install_next_steps() {
    echo "" >&3
    echo "What happens after installation:" >&3
    echo "  1. ${SERVER_KEY} was installed as MCP server command: ${MCP_ENTRYPOINT}" >&3
    if [ "$PROFILE" = "server" ]; then
        echo "  2. Generate a token (openssl rand -hex 32) and set AGENTIC_TOKEN on the server." >&3
        echo "  3. If artifacts were created on another machine, sync .amem/.acb/.avis files to this server." >&3
        echo "  4. Start MCP with auth, connect clients, then restart clients." >&3
        echo "  5. Optional feedback: open https://github.com/agentralabs/agentic-memory/issues" >&3
    else
        echo "  2. Restart your MCP client/system so it reloads MCP config." >&3
        echo "  3. ${SERVER_KEY} now auto-detects local .amem files at startup when available." >&3
        echo "  4. After restart, confirm '${SERVER_KEY}' appears in your MCP server list." >&3
        echo "  5. Optional feedback: open https://github.com/agentralabs/agentic-memory/issues" >&3
    fi
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
    set_progress 0 "Starting installer"
    echo "AgenticMemory Installer"
    echo "======================"
    echo ""

    set_progress 10 "Checking prerequisites"
    check_deps

    local platform
    set_progress 20 "Detecting platform"
    platform="$(detect_platform)"
    echo "Platform: ${platform}"
    validate_profile
    echo "Profile: ${PROFILE}"

    set_progress 30 "Resolving release"
    local installed_from_release=false
    if [ "$VERSION" = "latest" ]; then
        VERSION="$(get_latest_version)"
    fi

    if [ -n "$VERSION" ] && [ "$VERSION" != "null" ]; then
        echo "Version: ${VERSION}"
        if download_binary "$VERSION" "$platform"; then
            installed_from_release=true
            set_progress 70 "Release binary installed"
        else
            echo "Release artifact not found for ${VERSION}/${platform}; using source fallback."
        fi
    else
        echo "No GitHub release found; using source fallback."
    fi

    if [ "$installed_from_release" = false ]; then
        install_from_source
    fi

    set_progress 88 "Installing MCP launcher"
    install_mcp_entrypoint

    set_progress 90 "Applying profile setup"
    if [ "$PROFILE" = "desktop" ] || [ "$PROFILE" = "terminal" ]; then
        echo ""
        echo "Configuring MCP clients..."
        configure_mcp_clients
        print_client_help
    else
        print_terminal_server_help
    fi

    print_profile_help

    set_progress 100 "Install complete"
    finish_progress
    echo "Install complete: AgenticMemory (${PROFILE})" >&3
    echo "" >&3
    echo "Done! Memory defaults to ~/.brain.amem" >&3
    if [ "$PROFILE" = "desktop" ]; then
        echo "Restart any configured MCP client to activate." >&3
    fi
    print_post_install_next_steps

    check_path
}

main "$@"
