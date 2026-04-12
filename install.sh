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

echo ""
echo -e "Try it: ${BLUE}arbor --compact .${RESET}"
