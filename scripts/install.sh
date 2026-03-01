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
INSTALL_DIR_EXPLICIT=false
VERSION="latest"
PROFILE="${AGENTRA_INSTALL_PROFILE:-desktop}"
DRY_RUN=false
BAR_ONLY="${AGENTRA_INSTALL_BAR_ONLY:-1}"
MCP_ENTRYPOINT=""
HOST_OS=""
SERVER_ARGS_JSON='["serve"]'
SERVER_ARGS_TEXT='["serve"]'
SERVER_CHECK_CMD_SUFFIX=" serve"
MCP_CONFIGURED_CLIENTS=()
MCP_SCANNED_CONFIG_FILES=()

# ── Parse arguments ──────────────────────────────────────────────────
while [ $# -gt 0 ]; do
    case "$1" in
        --version=*)
            VERSION="${1#*=}"
            shift
            ;;
        --version)
            VERSION="${2:-}"
            shift 2
            ;;
        --dir=*)
            INSTALL_DIR="${1#*=}"
            INSTALL_DIR_EXPLICIT=true
            shift
            ;;
        --dir)
            INSTALL_DIR="${2:-}"
            INSTALL_DIR_EXPLICIT=true
            shift 2
            ;;
        --profile=*)
            PROFILE="${1#*=}"
            shift
            ;;
        --profile)
            PROFILE="${2:-}"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --help|-h)
            echo "Usage: install.sh [--version X.Y.Z|--version=X.Y.Z] [--dir /path|--dir=/path] [--profile desktop|terminal|server|--profile=desktop|terminal|server] [--dry-run]"
            exit 0
            ;;
        *)
            echo "Error: unknown option '$1'" >&2
            exit 1
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
        msys*|mingw*|cygwin*|windows_nt) os="windows" ;;
        *)      echo "Error: Unsupported OS: $os" >&2; exit 1 ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *)             echo "Error: Unsupported architecture: $arch" >&2; exit 1 ;;
    esac

    HOST_OS="$os"
    echo "${os}-${arch}"
}

# ── Check dependencies ────────────────────────────────────────────────
check_deps() {
    if ! command -v curl &>/dev/null; then
        echo "Error: 'curl' is required but not installed." >&2
        exit 1
    fi
    if ! command -v jq &>/dev/null && ! command -v python3 &>/dev/null; then
        echo "Error: JSON merge requires 'jq' or 'python3'." >&2
        echo "  Install jq (preferred) or python3, then rerun." >&2
        exit 1
    fi
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

    if ! command -v cargo &>/dev/null; then
        echo "Error: release artifacts are unavailable and cargo is not installed." >&2
        echo "Install Rust/Cargo first: https://rustup.rs" >&2
        exit 1
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

pwd_is_project() {
    [ -d "\$PWD/.git" ] || \
    [ -f "\$PWD/Cargo.toml" ] || \
    [ -f "\$PWD/package.json" ] || \
    [ -f "\$PWD/pyproject.toml" ] || \
    [ -f "\$PWD/go.mod" ] || \
    [ -d "\$PWD/src" ]
}

pwd_contains_projects() {
    find "\$PWD" -maxdepth 2 -type f \
        \\( -name 'Cargo.toml' -o -name 'package.json' -o -name 'pyproject.toml' -o -name 'go.mod' \\) \
        -print -quit 2>/dev/null | grep -q .
}

is_common_root_dir() {
    [ "\$PWD" = "\$HOME" ] || \
    [ "\$PWD" = "\$HOME/Documents" ] || \
    [ "\$PWD" = "\$HOME/Desktop" ] || \
    [ "\$PWD" = "/" ]
}

resolve_repo_root() {
    if [ -n "\${AGENTRA_WORKSPACE_ROOT:-}" ] && [ -d "\${AGENTRA_WORKSPACE_ROOT}" ]; then
        printf '%s' "\${AGENTRA_WORKSPACE_ROOT}"
        return
    fi
    if [ -n "\${AGENTRA_PROJECT_ROOT:-}" ] && [ -d "\${AGENTRA_PROJECT_ROOT}" ]; then
        printf '%s' "\${AGENTRA_PROJECT_ROOT}"
        return
    fi
    if command -v git >/dev/null 2>&1; then
        local root
        root="\$(git rev-parse --show-toplevel 2>/dev/null || true)"
        if [ -n "\$root" ] && [ -d "\$root" ]; then
            if [ "\$root" != "\$HOME" ] && [ "\$root" != "\$HOME/Documents" ] && [ "\$root" != "\$HOME/Desktop" ] && [ "\$root" != "/" ]; then
                printf '%s' "\$root"
                return
            fi
        fi
    fi
    if pwd_is_project; then
        printf '%s' "\$PWD"
        return
    fi
    if ! is_common_root_dir && pwd_contains_projects; then
        printf '%s' "\$PWD"
        return
    fi
    if command -v git >/dev/null 2>&1; then
        local root_fallback
        root_fallback="\$(git rev-parse --show-toplevel 2>/dev/null || true)"
        if [ -n "\$root_fallback" ] && [ -d "\$root_fallback" ]; then
            if [ "\$root_fallback" = "\$HOME" ] || [ "\$root_fallback" = "\$HOME/Documents" ] || [ "\$root_fallback" = "\$HOME/Desktop" ] || [ "\$root_fallback" = "/" ]; then
                printf '%s' "\$PWD"
                return
            fi
            printf '%s' "\$root_fallback"
            return
        fi
    fi
    printf '%s' "\$PWD"
}

slugify() {
    local raw="\$1"
    local base
    base="\$(basename "\$raw")"
    base="\$(printf '%s' "\$base" | tr '[:upper:]' '[:lower:]')"
    base="\$(printf '%s' "\$base" | sed -E 's/[^a-z0-9._-]+/-/g; s/^-+//; s/-+\$//')"
    if [ -z "\$base" ]; then
        base="workspace"
    fi
    printf '%s' "\$base"
}

find_memory() {
    local candidate
    local found=""
    local repo_root repo_slug

    repo_root="\$(resolve_repo_root)"
    repo_slug="\$(slugify "\$repo_root")"

    for candidate in \
        "\${AGENTRA_AMEM_PATH:-}" \
        "\${AGENTRA_MEMORY_PATH:-}" \
        "\${repo_root}/.agentra/\${repo_slug}.amem" \
        "\${repo_root}/.brain.amem" \
        "\$PWD/.brain.amem" \
        "\$PWD/memory.amem" \
        "\$HOME/.brain.amem" \
        "\$HOME/.agentra/memory/default.amem"; do
        if [ -n "\$candidate" ] && [ -f "\$candidate" ]; then
            found="\$candidate"
            break
        fi
    done

    if [ -n "\$found" ]; then
        printf '%s' "\$found"
        return
    fi
    mkdir -p "\${repo_root}/.agentra" >/dev/null 2>&1 || true
    printf '%s' "\${repo_root}/.agentra/\${repo_slug}.amem"
}

args=("\$@")
has_memory=0
has_command=0
serve_requested=0

for arg in "\${args[@]}"; do
    case "\$arg" in
        -h|--help|-V|--version) has_command=1 ;;
        -m|--memory|--memory=*) has_memory=1 ;;
        serve) has_command=1; serve_requested=1 ;;
        serve-http|validate|info|delete|export|compact|stats|status|extract|replay|daemon|help) has_command=1 ;;
    esac
done

if [ "\$has_command" -eq 0 ]; then
    serve_requested=1
    args+=("serve")
fi

if [ "\$serve_requested" -eq 1 ]; then
    if [ "\${AGENTRA_RUNTIME_MODE:-}" = "server" ] || [ "\${AGENTRA_SERVER:-}" = "1" ] || [ "\${AGENTRA_INSTALL_PROFILE:-}" = "server" ]; then
        if [ -z "\${AGENTIC_TOKEN:-}" ] && [ -z "\${AGENTIC_TOKEN_FILE:-}" ] && [ -z "\${AGENTRA_AUTH_TOKEN_FILE:-}" ]; then
            echo "Error: server mode requires AGENTIC_TOKEN or AGENTIC_TOKEN_FILE." >&2
            exit 2
        fi
    fi
fi

if [ "\$has_memory" -eq 0 ]; then
    memory_path="\$(find_memory || true)"
    if [ -n "\$memory_path" ]; then
        args=(--memory "\$memory_path" "\${args[@]}")
    fi
fi

exec "\$BIN" "\${args[@]}"
EOF
    chmod +x "${MCP_ENTRYPOINT}"
}

merge_config_with_python() {
    local config_file="$1"
    python3 - "$config_file" "$SERVER_KEY" "$MCP_ENTRYPOINT" "$SERVER_ARGS_JSON" <<'PY'
import json
import os
import sys

path, key, command, args_json = sys.argv[1:]
args = json.loads(args_json)
cfg = {}

if os.path.exists(path) and os.path.getsize(path) > 0:
    try:
        with open(path, "r", encoding="utf-8") as handle:
            loaded = json.load(handle)
        if isinstance(loaded, dict):
            cfg = loaded
    except Exception:
        cfg = {}

mcp = cfg.get("mcpServers")
if not isinstance(mcp, dict):
    mcp = {}
cfg["mcpServers"] = mcp
mcp[key] = {"command": command, "args": args}

os.makedirs(os.path.dirname(path), exist_ok=True)
tmp = f"{path}.tmp"
with open(tmp, "w", encoding="utf-8") as handle:
    json.dump(cfg, handle, indent=2)
    handle.write("\n")
os.replace(tmp, path)
PY
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

    if command -v jq >/dev/null 2>&1; then
        if [ -f "$config_file" ] && [ -s "$config_file" ]; then
            echo "    Existing config found, merging..."
            if jq --arg key "$SERVER_KEY" \
               --arg cmd "${MCP_ENTRYPOINT}" \
               --argjson args "$SERVER_ARGS_JSON" \
               '.mcpServers //= {} | .mcpServers[$key] = {"command": $cmd, "args": $args}' \
               "$config_file" > "$config_file.tmp"; then
                mv "$config_file.tmp" "$config_file"
            else
                rm -f "$config_file.tmp"
                echo "    jq merge failed; retrying with python3 fallback..."
                merge_config_with_python "$config_file"
            fi
        else
            echo "    Creating new config..."
            jq -n --arg key "$SERVER_KEY" \
                  --arg cmd "${MCP_ENTRYPOINT}" \
                  --argjson args "$SERVER_ARGS_JSON" \
               '{ "mcpServers": { ($key): { "command": $cmd, "args": $args } } }' \
               > "$config_file"
        fi
    else
        echo "    Merging with python3 fallback..."
        merge_config_with_python "$config_file"
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
    case "${HOST_OS}" in
        Darwin) config_file="$HOME/Library/Application Support/Claude/claude_desktop_config.json" ;;
        darwin) config_file="$HOME/Library/Application Support/Claude/claude_desktop_config.json" ;;
        Linux|linux)  config_file="${XDG_CONFIG_HOME:-$HOME/.config}/Claude/claude_desktop_config.json" ;;
        windows) config_file="${APPDATA:-$HOME/AppData/Roaming}/Claude/claude_desktop_config.json" ;;
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
        "${APPDATA:-$HOME/AppData/Roaming}"
        "${LOCALAPPDATA:-$HOME/AppData/Local}"
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
    if [ "${HOST_OS}" = "darwin" ]; then
        configure_json_client_if_present "VS Code" "$HOME/Library/Application Support/Code/User/mcp.json" "$HOME/Library/Application Support/Code/User"
        configure_json_client_if_present "VS Code + Cline" "$HOME/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json" "$HOME/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev"
        configure_json_client_if_present "VSCodium" "$HOME/Library/Application Support/VSCodium/User/mcp.json" "$HOME/Library/Application Support/VSCodium/User"
    elif [ "${HOST_OS}" = "windows" ]; then
        configure_json_client_if_present "Cursor" "${APPDATA:-$HOME/AppData/Roaming}/Cursor/User/mcp.json" "${APPDATA:-$HOME/AppData/Roaming}/Cursor/User"
        configure_json_client_if_present "Windsurf" "${APPDATA:-$HOME/AppData/Roaming}/Windsurf/User/mcp.json" "${APPDATA:-$HOME/AppData/Roaming}/Windsurf/User"
        configure_json_client_if_present "VS Code" "${APPDATA:-$HOME/AppData/Roaming}/Code/User/mcp.json" "${APPDATA:-$HOME/AppData/Roaming}/Code/User"
        configure_json_client_if_present "VS Code + Cline" "${APPDATA:-$HOME/AppData/Roaming}/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json" "${APPDATA:-$HOME/AppData/Roaming}/Code/User/globalStorage/saoudrizwan.claude-dev"
        configure_json_client_if_present "VSCodium" "${APPDATA:-$HOME/AppData/Roaming}/VSCodium/User/mcp.json" "${APPDATA:-$HOME/AppData/Roaming}/VSCodium/User"
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

maybe_install_daemon() {
    if [ "$PROFILE" = "server" ]; then
        echo "" >&3
        echo "Daemon auto-install skipped for server profile." >&3
        return
    fi

    local decision="${AGENTRA_INSTALL_DAEMON:-auto}"
    local should_install=true

    case "${decision}" in
        0|false|FALSE|no|NO|n|N)
            should_install=false
            ;;
        1|true|TRUE|yes|YES|y|Y)
            should_install=true
            ;;
        auto)
            if [ -t 0 ]; then
                printf "Install background memory daemon (recommended) [Y/n] " >&3
                local reply
                IFS= read -r reply || true
                case "${reply}" in
                    n|N|no|NO)
                        should_install=false
                        ;;
                    *)
                        should_install=true
                        ;;
                esac
            else
                should_install=true
            fi
            ;;
        *)
            should_install=true
            ;;
    esac

    if [ "$should_install" = false ]; then
        echo "Daemon installation skipped. Run '${INSTALL_DIR}/${BINARY_NAME} daemon install' anytime." >&3
        return
    fi

    if [ "$DRY_RUN" = true ]; then
        echo "  [dry-run] Would run: ${INSTALL_DIR}/${BINARY_NAME} daemon install" >&3
        return
    fi

    echo "Installing daemon service..." >&3
    if "${INSTALL_DIR}/${BINARY_NAME}" daemon install >/dev/null 2>&1; then
        echo "Daemon installed and will start automatically on login." >&3
    else
        echo "Warning: daemon install failed. You can retry with:" >&3
        echo "  ${INSTALL_DIR}/${BINARY_NAME} daemon install" >&3
    fi
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
        echo "  4. Daemon commands: ${INSTALL_DIR}/${BINARY_NAME} daemon status|logs|stop|start." >&3
        echo "  5. After restart, confirm '${SERVER_KEY}' appears in your MCP server list." >&3
        echo "  6. Optional feedback: open https://github.com/agentralabs/agentic-memory/issues" >&3
    fi
}

# ── Check PATH ────────────────────────────────────────────────────────
check_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo ""
        echo "Note: Add ${INSTALL_DIR} to your PATH if not already:"
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo ""
        echo "Add this line to your shell profile to make it permanent."
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
    HOST_OS="${platform%%-*}"
    echo "Platform: ${platform}"
    validate_profile
    echo "Profile: ${PROFILE}"
    if [ "${HOST_OS}" = "windows" ] && [ "$INSTALL_DIR_EXPLICIT" = false ]; then
        INSTALL_DIR="${HOME}/.agentra/bin"
    fi
    MCP_ENTRYPOINT="${INSTALL_DIR}/${BINARY_NAME}-agentra"
    echo "Install dir: ${INSTALL_DIR}"

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
    maybe_install_daemon

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
