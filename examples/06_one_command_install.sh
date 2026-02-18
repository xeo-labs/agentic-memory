#!/bin/bash
# ──────────────────────────────────────────────────────────────
# 06_one_command_install.sh — One-command install for AgenticMemory
#
# This script demonstrates the recommended install flow:
#   1. Install the Python package from PyPI
#   2. Run the auto-installer to download the platform binary
#   3. Verify the installation
#
# Usage:
#   chmod +x examples/06_one_command_install.sh
#   ./examples/06_one_command_install.sh
# ──────────────────────────────────────────────────────────────

set -euo pipefail

echo "=== Step 1: Install the agentic-memory Python package ==="
pip install agentic-memory

echo ""
echo "=== Step 2: Run the auto-installer (downloads platform binary) ==="
amem-install --auto --yes

echo ""
echo "=== Step 3: Verify installation ==="
amem-install status

echo ""
echo "Done. You can now use AgenticMemory in your Python scripts."
echo "Try running: python examples/01_basic_usage.py"
