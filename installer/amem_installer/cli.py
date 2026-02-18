"""CLI entry point — install, status, uninstall, update commands."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

from amem_installer.backup import list_backups
from amem_installer.brain_setup import BrainSetup
from amem_installer.configurators import ConfigResult, get_configurator, ConfigReport
from amem_installer.display import Display
from amem_installer.platform import PlatformInfo
from amem_installer.scanner import Scanner, ToolStatus


def _find_amem_binary() -> Path:
    """Find the amem CLI binary."""
    import shutil
    # Check PATH
    which = shutil.which("amem")
    if which:
        return Path(which)
    # Check common locations
    for p in [
        Path.home() / ".cargo" / "bin" / "amem",
        Path("/usr/local/bin/amem"),
    ]:
        if p.is_file() and os.access(str(p), os.X_OK):
            return p
    # Check env
    env = os.environ.get("AMEM_BINARY")
    if env:
        p = Path(env)
        if p.is_file():
            return p
    raise FileNotFoundError("amem binary not found. Install AgenticMemory core first.")


def _format_size(size_bytes: int) -> str:
    """Format file size in human-readable form."""
    if size_bytes < 1024:
        return f"{size_bytes} B"
    elif size_bytes < 1024 * 1024:
        return f"{size_bytes / 1024:.1f} KB"
    elif size_bytes < 1024 * 1024 * 1024:
        return f"{size_bytes / (1024 * 1024):.1f} MB"
    else:
        return f"{size_bytes / (1024 * 1024 * 1024):.1f} GB"


# ===================================================================
# install
# ===================================================================

def cmd_install(args: argparse.Namespace) -> int:
    """Main install command."""
    platform = PlatformInfo.detect()
    display = Display()

    display.header("AgenticMemory Installer")

    # Find amem binary
    try:
        amem_binary = _find_amem_binary()
    except FileNotFoundError as e:
        display.fail("Setup", str(e))
        return 1

    # Scan
    display.section("Scanning for AI tools...")
    scanner = Scanner(platform)
    tools = scanner.scan()
    display.scan_results(tools)

    found = [t for t in tools if t.status in (ToolStatus.FOUND, ToolStatus.RUNNING)]
    already = [t for t in tools if t.status == ToolStatus.ALREADY_CONFIGURED]

    if not found and not already:
        display.warning("No configurable AI tools found on this machine.")
        display.info("Install Claude Code, Cursor, Ollama, or other supported tools first.")
        return 1

    if args.list:
        return 0

    # Filter tools
    if args.only:
        only_ids = [t.strip() for t in args.only.split(",")]
        found = [t for t in found if t.tool_id in only_ids]
    if args.skip:
        skip_ids = [t.strip() for t in args.skip.split(",")]
        found = [t for t in found if t.tool_id not in skip_ids]

    if not found:
        display.info("No tools to configure (all skipped or already configured).")
        if already:
            display.info(f"{len(already)} tool(s) already configured.")
        return 0

    # Confirm
    if not args.yes and not args.dry_run:
        display.section(f"Will configure {len(found)} tool(s):")
        for t in found:
            display.tool_line(t.name, t.config_path or "service integration")
        if not display.confirm("Proceed?"):
            display.info("Cancelled.")
            return 0

    # Ensure brain exists
    brain_path = Path(args.brain) if args.brain else platform.brain_path
    display.section("Setting up shared brain...")
    brain_setup = BrainSetup(brain_path, amem_binary)
    try:
        brain_setup.ensure_exists()
        display.success(f"Brain: {brain_path}")
    except RuntimeError as e:
        display.fail("Brain setup", str(e))
        return 1

    # Configure each tool
    display.section("Configuring tools...")
    reports: list[ConfigReport] = []

    for tool in found:
        configurator = get_configurator(tool)
        if configurator is None:
            display.skip(tool.name, "No configurator available")
            continue

        try:
            report = configurator.configure(
                tool=tool,
                brain_path=brain_path,
                amem_binary=amem_binary,
                dry_run=args.dry_run,
            )
            reports.append(report)

            if report.result == ConfigResult.SUCCESS:
                display.success(f"{tool.name} → {report.message}")
                if report.backup_path:
                    display.detail(f"Backup: {report.backup_path}")
            elif report.result == ConfigResult.ALREADY_CONFIGURED:
                display.already(tool.name)
            else:
                display.fail(tool.name, report.message)
        except Exception as e:
            display.fail(tool.name, str(e))
            reports.append(ConfigReport(
                tool_name=tool.name, tool_id=tool.tool_id,
                result=ConfigResult.FAILED,
                config_path=None, backup_path=None,
                message=str(e), restart_required=False,
            ))

    # Summary
    display.section("Summary")
    succeeded = [r for r in reports if r.result == ConfigResult.SUCCESS]
    failed = [r for r in reports if r.result == ConfigResult.FAILED]
    need_restart = [r for r in reports if r.restart_required]

    display.summary(
        configured=len(succeeded),
        already=len(already),
        failed=len(failed),
        brain_path=brain_path,
    )

    if need_restart:
        display.section("Restart required:")
        for r in need_restart:
            display.detail(f"  Restart {r.tool_name} to activate memory")

    display.info("\nRun 'amem status' to verify connections.")

    return 0 if not failed else 1


# ===================================================================
# status
# ===================================================================

def cmd_status(args: argparse.Namespace) -> int:
    """Show connection status for all configured tools."""
    platform = PlatformInfo.detect()
    display = Display()

    display.header("AgenticMemory Status")

    brain_path = platform.brain_path
    if brain_path.exists():
        try:
            amem_binary = _find_amem_binary()
            result = subprocess.run(
                [str(amem_binary), "--format", "json", "info", str(brain_path)],
                capture_output=True, text=True, timeout=10,
            )
            if result.returncode == 0:
                info = json.loads(result.stdout)
                display.section("Brain")
                display.info(f"  Path:     {brain_path}")
                display.info(f"  Nodes:    {info.get('nodes', info.get('node_count', 0)):,}")
                display.info(f"  Edges:    {info.get('edges', info.get('edge_count', 0)):,}")
                display.info(f"  Sessions: {info.get('sessions', info.get('session_count', 0)):,}")
                size = os.path.getsize(brain_path)
                display.info(f"  Size:     {_format_size(size)}")
            else:
                display.warning(f"Brain file exists but amem can't read it: {result.stderr.strip()}")
        except FileNotFoundError:
            display.warning("amem binary not found. Install AgenticMemory core.")
    else:
        display.warning(f"No brain file at {brain_path}")
        display.info("Run 'amem install' to set up.")
        return 1

    # Tool status
    display.section("Connected Tools")
    scanner = Scanner(platform)
    tools = scanner.scan()

    connected = 0
    for tool in tools:
        if tool.status == ToolStatus.ALREADY_CONFIGURED:
            configurator = get_configurator(tool)
            if configurator and configurator.verify(tool):
                display.success(f"{tool.name:20s} Connected")
                connected += 1
            else:
                display.warning(f"{tool.name:20s} Configured but verification failed")
        elif tool.status in (ToolStatus.FOUND, ToolStatus.RUNNING):
            display.skip(tool.name, "Detected but not configured")

    if connected == 0:
        display.info("\n  No tools connected. Run 'amem install' to set up.")
    else:
        display.info(f"\n  {connected} tool(s) connected to shared brain.")

    # Backup registry
    backups = list_backups()
    if backups:
        display.section("Backups")
        display.info(f"  {len(backups)} config backup(s) stored")
        for b in backups[-3:]:
            display.detail(f"{b.get('tool', 'unknown'):15s} {b.get('timestamp', '')[:19]}  {b.get('backup', '')}")

    return 0


# ===================================================================
# uninstall
# ===================================================================

def cmd_uninstall(args: argparse.Namespace) -> int:
    """Remove AgenticMemory from all configured tools."""
    platform = PlatformInfo.detect()
    display = Display()

    display.header("AgenticMemory Uninstall")

    scanner = Scanner(platform)
    tools = scanner.scan()
    configured = [t for t in tools if t.status == ToolStatus.ALREADY_CONFIGURED]

    if not configured:
        display.info("No tools are configured with AgenticMemory. Nothing to uninstall.")
        return 0

    display.section(f"Will remove AgenticMemory from {len(configured)} tool(s):")
    for t in configured:
        display.tool_line(t.name, t.config_path or "service")

    if not display.confirm("Proceed? This will restore original configs."):
        display.info("Cancelled.")
        return 0

    # Load backup registry
    backups = list_backups()
    backup_map: dict[str, str] = {}
    for b in backups:
        backup_map[b["original"]] = b["backup"]

    # Unconfigure each tool
    display.section("Removing configurations...")
    for tool in configured:
        configurator = get_configurator(tool)
        if configurator is None:
            display.skip(tool.name, "No configurator")
            continue

        backup_path = backup_map.get(str(tool.config_path)) if tool.config_path else None

        try:
            report = configurator.unconfigure(tool, backup_path)
            if report.result == ConfigResult.SUCCESS:
                if backup_path:
                    display.success(f"{tool.name} → Restored from backup")
                else:
                    display.success(f"{tool.name} → AgenticMemory entry removed")
            else:
                display.fail(tool.name, report.message)
        except Exception as e:
            display.fail(tool.name, str(e))

    # Clean up wrapper scripts
    amem_dir = platform.amem_dir
    for wrapper in ["ollama-amem", "ollama-amem.yaml", "lm-studio-amem.yaml"]:
        wp = amem_dir / wrapper
        if wp.exists():
            wp.unlink()

    display.section("Summary")
    display.info("AgenticMemory has been removed from all tools.")
    display.info(f"Brain file preserved at: {platform.brain_path}")
    display.info("Your memory data has NOT been deleted.")
    display.info("To delete all data: rm -rf ~/.amem/")

    return 0


# ===================================================================
# update
# ===================================================================

def cmd_update(args: argparse.Namespace) -> int:
    """Re-scan for tools and update configurations."""
    platform = PlatformInfo.detect()
    display = Display()

    display.header("AgenticMemory Update")

    brain_path = platform.brain_path
    if not brain_path.exists():
        display.warning("No brain file found. Run 'amem install' first.")
        return 1

    try:
        amem_binary = _find_amem_binary()
    except FileNotFoundError as e:
        display.fail("Setup", str(e))
        return 1

    display.section("Re-scanning for AI tools...")
    scanner = Scanner(platform)
    tools = scanner.scan()
    display.scan_results(tools)

    new_tools = [t for t in tools if t.status in (ToolStatus.FOUND, ToolStatus.RUNNING)]

    if not new_tools:
        display.info("\nAll detected tools are already configured. Nothing to update.")
        return 0

    display.section(f"Found {len(new_tools)} new tool(s) to configure:")
    for t in new_tools:
        display.tool_line(t.name, t.config_path or "service")

    if not display.confirm("Configure new tools?"):
        display.info("Cancelled.")
        return 0

    for tool in new_tools:
        configurator = get_configurator(tool)
        if configurator is None:
            continue

        try:
            report = configurator.configure(
                tool=tool, brain_path=brain_path, amem_binary=amem_binary,
            )
            if report.result == ConfigResult.SUCCESS:
                display.success(f"{tool.name} → {report.message}")
            else:
                display.fail(tool.name, report.message)
        except Exception as e:
            display.fail(tool.name, str(e))

    return 0


# ===================================================================
# Entry point
# ===================================================================

def main() -> int:
    """CLI entry point."""
    parser = argparse.ArgumentParser(
        prog="amem",
        description="AgenticMemory Installer — Connect all your AI tools to shared memory",
    )
    subparsers = parser.add_subparsers(dest="command")

    # install
    install_parser = subparsers.add_parser("install", help="Install AgenticMemory to detected tools")
    install_parser.add_argument("--auto", action="store_true", default=True, help="Auto-detect and configure")
    install_parser.add_argument("--only", help="Only configure these tools (comma-separated IDs)")
    install_parser.add_argument("--skip", help="Skip these tools (comma-separated IDs)")
    install_parser.add_argument("--brain", help="Brain file path (default: ~/.amem/brain.amem)")
    install_parser.add_argument("--dry-run", action="store_true", help="Show what would be done")
    install_parser.add_argument("--yes", "-y", action="store_true", help="Skip confirmation")
    install_parser.add_argument("--verbose", "-v", action="store_true")
    install_parser.add_argument("--list", action="store_true", help="List detected tools and exit")

    # status
    subparsers.add_parser("status", help="Show connection status")

    # uninstall
    subparsers.add_parser("uninstall", help="Remove AgenticMemory from all tools")

    # update
    subparsers.add_parser("update", help="Re-scan and update configurations")

    args = parser.parse_args()

    if args.command == "install":
        return cmd_install(args)
    elif args.command == "status":
        return cmd_status(args)
    elif args.command == "uninstall":
        return cmd_uninstall(args)
    elif args.command == "update":
        return cmd_update(args)
    else:
        parser.print_help()
        return 0


if __name__ == "__main__":
    sys.exit(main())
