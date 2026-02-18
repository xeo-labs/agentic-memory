# AgenticMemory Installer

One-command installer that detects every AI tool on your machine and connects them all to a shared AgenticMemory brain.

## Install

```bash
pip install amem-installer
```

## Usage

```bash
# Auto-detect and configure all tools
amem-install --auto

# Show what would be done (dry run)
amem-install install --dry-run

# Check connection status
amem-install status

# Remove all configurations
amem-install uninstall

# Re-scan for new tools
amem-install update
```

## Supported Tools

| Tool | Detection | Integration |
|:---|:---|:---|
| Claude Code | Config file | MCP server |
| Cursor | Config file | MCP server |
| Windsurf | Config file | MCP server |
| Claude Desktop | Config file | MCP server |
| Continue | Config file | Context provider |
| OpenClaw | Config file | YAML config |
| Ollama | HTTP service | Wrapper script |
| LM Studio | HTTP service | Config file |
| LangChain | requirements.txt | Instructions |
| CrewAI | requirements.txt | Instructions |
| AutoGen | requirements.txt | Instructions |

## How It Works

1. Scans your system for installed AI tools
2. Creates a shared brain file at `~/.amem/brain.amem`
3. Configures each tool to use the shared brain (via MCP, config files, or wrapper scripts)
4. Backs up all modified configs before changes

All modifications are additive — existing configurations are never deleted.

## Tests

39 tests passing. All tests use sandboxed home directories.

```bash
pip install -e ".[dev]"
pytest tests/ -v
```

## License

MIT
