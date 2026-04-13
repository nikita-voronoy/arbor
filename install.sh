#!/usr/bin/env bash
set -euo pipefail

REPO="nikita-voronoy/arbor"
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

echo -e "${BLUE}arbor${RESET} — code navigation MCP server"
echo ""

# --- Detect OS/arch ---
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)   os="unknown-linux-gnu" ;;
  Darwin)  os="apple-darwin" ;;
  MINGW*|MSYS*|CYGWIN*) os="pc-windows-msvc" ;;
  *) err "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64)  arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *) err "Unsupported architecture: $ARCH"; exit 1 ;;
esac

TARGET="${arch}-${os}"

# --- Find latest release ---
info "Fetching latest release..."

if command -v curl &>/dev/null; then
  FETCH="curl -fsSL"
elif command -v wget &>/dev/null; then
  FETCH="wget -qO-"
else
  err "Neither curl nor wget found"
  exit 1
fi

LATEST=$($FETCH "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
  | grep '"tag_name"' | head -1 | sed -E 's/.*"([^"]+)".*/\1/') || true

if [ -z "$LATEST" ]; then
  echo ""
  info "No release found — falling back to cargo install..."
  if ! command -v cargo &>/dev/null; then
    err "cargo not found. Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
  fi
  cargo install --git "https://github.com/${REPO}.git" arbor-mcp
  ok "arbor installed via cargo → $(which arbor)"
else
  # --- Download binary ---
  if [ "$os" = "pc-windows-msvc" ]; then
    ASSET="${BIN}-${TARGET}.zip"
  else
    ASSET="${BIN}-${TARGET}.tar.gz"
  fi

  URL="https://github.com/${REPO}/releases/download/${LATEST}/${ASSET}"
  info "Downloading ${LATEST} for ${TARGET}..."

  TMPDIR="$(mktemp -d)"
  trap 'rm -rf "$TMPDIR"' EXIT

  $FETCH "$URL" > "$TMPDIR/$ASSET" 2>/dev/null || {
    err "Failed to download $URL"
    info "Falling back to cargo install..."
    if command -v cargo &>/dev/null; then
      cargo install --git "https://github.com/${REPO}.git" arbor-mcp
      ok "arbor installed via cargo → $(which arbor)"
    else
      err "cargo not found. Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
      exit 1
    fi
    LATEST=""
  }

  if [ -n "$LATEST" ]; then
    # Extract
    if [ "$os" = "pc-windows-msvc" ]; then
      unzip -q "$TMPDIR/$ASSET" -d "$TMPDIR"
    else
      tar xzf "$TMPDIR/$ASSET" -C "$TMPDIR"
    fi

    # Install
    mkdir -p "$INSTALL_DIR"
    mv "$TMPDIR/$BIN" "$INSTALL_DIR/$BIN"
    chmod +x "$INSTALL_DIR/$BIN"

    ok "arbor ${LATEST} installed → ${INSTALL_DIR}/${BIN}"

    # Check PATH
    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
      echo ""
      echo "Add to your PATH:"
      echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
      echo ""
    fi
  fi
fi

echo ""

# --- Configure Claude Code ---
if command -v claude &>/dev/null; then
  info "Adding arbor to Claude Code..."
  claude mcp add arbor -- "$BIN" 2>/dev/null && ok "Registered with Claude Code." || info "Already registered or claude mcp not available."
else
  echo "To add to Claude Code later:"
  echo "  claude mcp add arbor -- arbor"
fi

# --- Configure hooks to prefer arbor MCP ---
SETTINGS="$HOME/.claude/settings.json"

if command -v jq &>/dev/null; then
  info "Configuring PreToolUse hook for arbor preference..."

  mkdir -p "$HOME/.claude"

  HOOK_ENTRY='{
    "matcher": "Grep|Glob",
    "hooks": [{
      "type": "command",
      "command": "echo '\''{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"additionalContext\":\"STOP: Prefer arbor MCP tools (search, references, skeleton, compact, boot) over Grep/Glob for code navigation. Fall back to Grep/Glob only for string literals, comments, or regex patterns.\"}}'\''",
      "statusMessage": "Checking arbor preference..."
    }]
  }'

  if [ -f "$SETTINGS" ]; then
    # Check if a Grep|Glob hook already exists
    if jq -e '.hooks.PreToolUse[]? | select(.matcher == "Grep|Glob")' "$SETTINGS" &>/dev/null; then
      info "PreToolUse hook for Grep|Glob already configured — skipping."
    else
      # Merge into existing settings
      jq --argjson hook "$HOOK_ENTRY" '.hooks.PreToolUse = ((.hooks.PreToolUse // []) + [$hook])' "$SETTINGS" > "$SETTINGS.tmp" \
        && mv "$SETTINGS.tmp" "$SETTINGS"
      ok "PreToolUse hook added to $SETTINGS"
    fi
  else
    # Create settings.json with just the hook
    jq -n --argjson hook "$HOOK_ENTRY" '{"hooks":{"PreToolUse":[$hook]}}' > "$SETTINGS"
    ok "Created $SETTINGS with PreToolUse hook"
  fi
else
  info "jq not found — skipping hook configuration."
  info "To manually configure, add a PreToolUse hook for Grep|Glob in $SETTINGS"
fi

# --- Add CLAUDE.md instructions ---
CLAUDE_MD="$HOME/.claude/CLAUDE.md"

MARKER_START='<!-- arbor:start -->'
MARKER_END='<!-- arbor:end -->'

ARBOR_BLOCK="${MARKER_START}
## Code navigation: use arbor MCP first

When exploring a codebase or searching for code, **always prefer arbor MCP tools over grep/glob**:

- **Instead of grep for a symbol** → use \`mcp__arbor__search\` (fuzzy, deduped, ranked)
- **Instead of grep for \"who calls X\"** → use \`mcp__arbor__references\` (shows Definition, Call, TypeReference)
- **Instead of reading many files to understand structure** → use \`mcp__arbor__boot\` first, then \`mcp__arbor__skeleton\` or \`mcp__arbor__compact\`
- **Instead of manually tracing dependencies** → use \`mcp__arbor__dependencies\` or \`mcp__arbor__impact\`
- **After making changes to many files** → call \`mcp__arbor__reindex\` to refresh the index

Start every new project session with \`mcp__arbor__boot\` to get the project overview.

Always try arbor first, even for terms that might appear in comments or string literals. Fall back to grep/glob only when:
- arbor is not available
- arbor returned nothing useful and you need raw text/regex search as a last resort
${MARKER_END}"

if [ -f "$CLAUDE_MD" ]; then
  if grep -q "$MARKER_START" "$CLAUDE_MD"; then
    info "CLAUDE.md already contains arbor block — replacing..."
    # Remove old block and insert new one
    sed "/$MARKER_START/,/$MARKER_END/d" "$CLAUDE_MD" > "$CLAUDE_MD.tmp"
    printf '%s\n' "$ARBOR_BLOCK" >> "$CLAUDE_MD.tmp"
    mv "$CLAUDE_MD.tmp" "$CLAUDE_MD"
    ok "Arbor block updated in $CLAUDE_MD"
  else
    info "Appending arbor instructions to $CLAUDE_MD..."
    printf '\n%s\n' "$ARBOR_BLOCK" >> "$CLAUDE_MD"
    ok "Arbor instructions added to $CLAUDE_MD"
  fi
else
  mkdir -p "$HOME/.claude"
  printf '%s\n' "$ARBOR_BLOCK" > "$CLAUDE_MD"
  ok "Created $CLAUDE_MD with arbor instructions"
fi

echo ""
echo -e "Try it: ${BLUE}arbor --compact .${RESET}"
