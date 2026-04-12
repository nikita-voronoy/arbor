#!/usr/bin/env bash
set -euo pipefail

REPO="https://github.com/nikita-voronoy/arbor.git"
BIN="arbor"
BLUE='\033[0;34m'
GREEN='\033[0;32m'
DIM='\033[2m'
RESET='\033[0m'

echo -e "${BLUE}arbor${RESET} — code navigation MCP server"
echo ""

# --- 1. Check cargo ---
if ! command -v cargo &>/dev/null; then
  echo "cargo not found. Install Rust first:"
  echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  exit 1
fi

# --- 2. Install binary ---
echo -e "${DIM}Installing arbor via cargo...${RESET}"
cargo install --git "$REPO" arbor-mcp 2>&1 | tail -3
echo ""

# Verify
if ! command -v "$BIN" &>/dev/null; then
  echo "Binary not found in PATH. Make sure ~/.cargo/bin is in your PATH."
  exit 1
fi

echo -e "${GREEN}arbor installed${RESET} → $(which arbor)"
echo ""

# --- 3. Configure Claude Code MCP ---
if command -v claude &>/dev/null; then
  echo -e "${DIM}Adding arbor to Claude Code...${RESET}"
  claude mcp add arbor -- arbor
  echo -e "${GREEN}Done.${RESET} arbor is now available in Claude Code."
else
  echo "Claude Code CLI not found — add manually to your MCP config:"
  echo ""
  echo '  claude mcp add arbor -- arbor'
  echo ""
  echo "Or add to ~/.claude.json:"
  echo ""
  echo '  { "mcpServers": { "arbor": { "command": "arbor" } } }'
fi

echo ""
echo -e "Try it: ${BLUE}arbor --compact .${RESET}"
