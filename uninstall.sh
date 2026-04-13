#!/usr/bin/env bash
set -euo pipefail

BIN="arbor"
INSTALL_DIR="${ARBOR_INSTALL_DIR:-$HOME/.local/bin}"

BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
DIM='\033[2m'
RESET='\033[0m'

info()  { echo -e "${DIM}$*${RESET}"; }
ok()    { echo -e "${GREEN}$*${RESET}"; }
err()   { echo -e "${RED}$*${RESET}" >&2; }

echo -e "${BLUE}arbor${RESET} — uninstaller"
echo ""

# --- Remove binary ---
if [ -f "$INSTALL_DIR/$BIN" ]; then
  rm "$INSTALL_DIR/$BIN"
  ok "Removed $INSTALL_DIR/$BIN"
elif command -v arbor &>/dev/null; then
  ARBOR_PATH="$(which arbor)"
  info "Found arbor at $ARBOR_PATH (not in expected $INSTALL_DIR)"
  info "Remove it manually: rm \"$ARBOR_PATH\""
else
  info "arbor binary not found — skipping."
fi

# --- Remove Claude Code MCP registration ---
if command -v claude &>/dev/null; then
  info "Removing arbor from Claude Code..."
  claude mcp remove arbor 2>/dev/null && ok "Unregistered from Claude Code." || info "Not registered or claude mcp not available."
fi

# --- Remove PreToolUse hook from settings.json ---
SETTINGS="$HOME/.claude/settings.json"

if [ -f "$SETTINGS" ] && command -v jq &>/dev/null; then
  if jq -e '.hooks.PreToolUse[]? | select(.matcher == "Grep|Glob")' "$SETTINGS" &>/dev/null; then
    info "Removing PreToolUse hook for Grep|Glob..."
    jq '(.hooks.PreToolUse) |= map(select(.matcher != "Grep|Glob"))
      | if (.hooks.PreToolUse | length) == 0 then del(.hooks.PreToolUse) else . end
      | if (.hooks | length) == 0 then del(.hooks) else . end' "$SETTINGS" > "$SETTINGS.tmp" \
      && mv "$SETTINGS.tmp" "$SETTINGS"
    ok "PreToolUse hook removed from $SETTINGS"
  else
    info "No arbor PreToolUse hook found in $SETTINGS — skipping."
  fi
fi

# --- Remove arbor block from CLAUDE.md ---
CLAUDE_MD="$HOME/.claude/CLAUDE.md"
MARKER_START='<!-- arbor:start -->'
MARKER_END='<!-- arbor:end -->'

if [ -f "$CLAUDE_MD" ]; then
  if grep -q "$MARKER_START" "$CLAUDE_MD"; then
    info "Removing arbor block from $CLAUDE_MD..."
    sed "/$MARKER_START/,/$MARKER_END/d" "$CLAUDE_MD" > "$CLAUDE_MD.tmp"
    # Clean up trailing blank lines
    perl -0777 -pe 's/\n+\z/\n/' "$CLAUDE_MD.tmp" > "$CLAUDE_MD"
    rm -f "$CLAUDE_MD.tmp"
    # If file is now empty or only whitespace, remove it
    if [ ! -s "$CLAUDE_MD" ] || ! grep -q '[^[:space:]]' "$CLAUDE_MD"; then
      rm "$CLAUDE_MD"
      ok "Removed empty $CLAUDE_MD"
    else
      ok "Arbor block removed from $CLAUDE_MD"
    fi
  else
    info "No arbor markers found in $CLAUDE_MD — skipping."
  fi
fi

echo ""
ok "arbor uninstalled."
