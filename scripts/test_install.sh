#!/usr/bin/env bash
set -euo pipefail

# Test suite for install.sh / uninstall.sh config logic
# Tests only the settings.json + CLAUDE.md portions (no binary download)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TMPROOT="$(mktemp -d)"
trap 'rm -rf "$TMPROOT"' EXIT

PASS=0
FAIL=0

pass() { PASS=$((PASS + 1)); echo "  PASS: $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  FAIL: $1"; }
assert_eq() {
  if [ "$1" = "$2" ]; then pass "$3"; else fail "$3 (expected '$2', got '$1')"; fi
}
assert_contains() {
  if echo "$1" | grep -qF "$2"; then pass "$3"; else fail "$3 (missing '$2')"; fi
}
assert_not_contains() {
  if echo "$1" | grep -qF "$2"; then fail "$3 (found '$2')"; else pass "$3"; fi
}
assert_file_exists() {
  if [ -f "$1" ]; then pass "$2"; else fail "$2 ($1 not found)"; fi
}
assert_file_not_exists() {
  if [ ! -f "$1" ]; then pass "$2"; else fail "$2 ($1 still exists)"; fi
}

# Helper: run install config section with fake HOME
run_install() {
  local home="$1"
  HOME="$home" bash -c '
    set -euo pipefail
    SETTINGS="$HOME/.claude/settings.json"
    CLAUDE_MD="$HOME/.claude/CLAUDE.md"

    HOOK_ENTRY='"'"'{
      "matcher": "Grep|Glob",
      "hooks": [{
        "type": "command",
        "command": "echo arbor-hook",
        "statusMessage": "Checking arbor preference..."
      }]
    }'"'"'

    mkdir -p "$HOME/.claude"

    if [ -f "$SETTINGS" ]; then
      if jq -e ".hooks.PreToolUse[]? | select(.matcher == \"Grep|Glob\")" "$SETTINGS" &>/dev/null; then
        : # skip
      else
        jq --argjson hook "$HOOK_ENTRY" ".hooks.PreToolUse = ((.hooks.PreToolUse // []) + [\$hook])" "$SETTINGS" > "$SETTINGS.tmp" \
          && mv "$SETTINGS.tmp" "$SETTINGS"
      fi
    else
      jq -n --argjson hook "$HOOK_ENTRY" "{\"hooks\":{\"PreToolUse\":[\$hook]}}" > "$SETTINGS"
    fi

    MARKER_START="<!-- arbor:start -->"
    MARKER_END="<!-- arbor:end -->"
    ARBOR_BLOCK="${MARKER_START}
## Code navigation: use arbor MCP first
Test content here.
${MARKER_END}"

    if [ -f "$CLAUDE_MD" ]; then
      if grep -q "$MARKER_START" "$CLAUDE_MD"; then
        sed "/$MARKER_START/,/$MARKER_END/d" "$CLAUDE_MD" > "$CLAUDE_MD.tmp"
        printf "%s\n" "$ARBOR_BLOCK" >> "$CLAUDE_MD.tmp"
        mv "$CLAUDE_MD.tmp" "$CLAUDE_MD"
      else
        printf "\n%s\n" "$ARBOR_BLOCK" >> "$CLAUDE_MD"
      fi
    else
      printf "%s\n" "$ARBOR_BLOCK" > "$CLAUDE_MD"
    fi
  '
}

# Helper: run uninstall config section with fake HOME
run_uninstall() {
  local home="$1"
  HOME="$home" bash -c '
    set -euo pipefail
    SETTINGS="$HOME/.claude/settings.json"
    CLAUDE_MD="$HOME/.claude/CLAUDE.md"
    MARKER_START="<!-- arbor:start -->"
    MARKER_END="<!-- arbor:end -->"

    if [ -f "$SETTINGS" ] && command -v jq &>/dev/null; then
      if jq -e ".hooks.PreToolUse[]? | select(.matcher == \"Grep|Glob\")" "$SETTINGS" &>/dev/null; then
        jq "(.hooks.PreToolUse) |= map(select(.matcher != \"Grep|Glob\"))
          | if (.hooks.PreToolUse | length) == 0 then del(.hooks.PreToolUse) else . end
          | if (.hooks | length) == 0 then del(.hooks) else . end" "$SETTINGS" > "$SETTINGS.tmp" \
          && mv "$SETTINGS.tmp" "$SETTINGS"
      fi
    fi

    if [ -f "$CLAUDE_MD" ]; then
      if grep -q "$MARKER_START" "$CLAUDE_MD"; then
        sed "/$MARKER_START/,/$MARKER_END/d" "$CLAUDE_MD" > "$CLAUDE_MD.tmp"
        # Trim trailing blank lines using perl (avoids sed $d escaping issues)
        perl -0777 -pe "s/\n+\z/\n/" "$CLAUDE_MD.tmp" > "$CLAUDE_MD"
        rm -f "$CLAUDE_MD.tmp"
        if [ ! -s "$CLAUDE_MD" ] || ! grep -q "[^[:space:]]" "$CLAUDE_MD"; then
          rm "$CLAUDE_MD"
        fi
      fi
    fi
  '
}

# ============================================================
echo "=== Test 1: Clean install (no existing files) ==="
FAKE="$TMPROOT/t1"
mkdir -p "$FAKE"
run_install "$FAKE"

assert_file_exists "$FAKE/.claude/settings.json" "settings.json created"
assert_file_exists "$FAKE/.claude/CLAUDE.md" "CLAUDE.md created"
assert_eq "$(jq -r '.hooks.PreToolUse[0].matcher' "$FAKE/.claude/settings.json")" "Grep|Glob" "hook matcher correct"
assert_contains "$(cat "$FAKE/.claude/CLAUDE.md")" "<!-- arbor:start -->" "CLAUDE.md has start marker"
assert_contains "$(cat "$FAKE/.claude/CLAUDE.md")" "<!-- arbor:end -->" "CLAUDE.md has end marker"
assert_contains "$(cat "$FAKE/.claude/CLAUDE.md")" "## Code navigation" "CLAUDE.md has section header"

# ============================================================
echo "=== Test 2: Idempotency (install twice) ==="
run_install "$FAKE"

HOOK_COUNT=$(jq '.hooks.PreToolUse | length' "$FAKE/.claude/settings.json")
assert_eq "$HOOK_COUNT" "1" "no duplicate hooks after second install"
MARKER_COUNT=$(grep -c 'arbor:start' "$FAKE/.claude/CLAUDE.md")
assert_eq "$MARKER_COUNT" "1" "no duplicate CLAUDE.md blocks after second install"

# ============================================================
echo "=== Test 3: Install preserves existing settings ==="
FAKE="$TMPROOT/t3"
mkdir -p "$FAKE/.claude"
cat > "$FAKE/.claude/settings.json" << 'EOF'
{
  "skipDangerousModePermissionPrompt": true,
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "echo logging"}]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [{"type": "command", "command": "prettier --write"}]
      }
    ]
  },
  "permissions": {"allow": ["Bash(git:*)"]}
}
EOF
run_install "$FAKE"

assert_eq "$(jq -r '.skipDangerousModePermissionPrompt' "$FAKE/.claude/settings.json")" "true" "preserves top-level settings"
assert_eq "$(jq -r '.permissions.allow[0]' "$FAKE/.claude/settings.json")" "Bash(git:*)" "preserves permissions"
assert_eq "$(jq '.hooks.PreToolUse | length' "$FAKE/.claude/settings.json")" "2" "two PreToolUse hooks total"
assert_eq "$(jq -r '.hooks.PreToolUse[0].matcher' "$FAKE/.claude/settings.json")" "Bash" "existing Bash hook preserved"
assert_eq "$(jq -r '.hooks.PreToolUse[1].matcher' "$FAKE/.claude/settings.json")" "Grep|Glob" "arbor hook added"
assert_eq "$(jq '.hooks.PostToolUse | length' "$FAKE/.claude/settings.json")" "1" "PostToolUse hooks preserved"

# ============================================================
echo "=== Test 4: Install preserves existing CLAUDE.md content ==="
FAKE="$TMPROOT/t4"
mkdir -p "$FAKE/.claude"
cat > "$FAKE/.claude/CLAUDE.md" << 'EOF'
# Global instructions

## My custom rules

- Always use TypeScript
- Follow ESLint config
EOF
run_install "$FAKE"

CONTENT=$(cat "$FAKE/.claude/CLAUDE.md")
assert_contains "$CONTENT" "My custom rules" "existing content preserved"
assert_contains "$CONTENT" "Always use TypeScript" "existing rules preserved"
assert_contains "$CONTENT" "<!-- arbor:start -->" "arbor block added"

# ============================================================
echo "=== Test 5: Install updates existing arbor block ==="
# Run again — the block should be replaced, not duplicated
run_install "$FAKE"

MARKER_COUNT=$(grep -c 'arbor:start' "$FAKE/.claude/CLAUDE.md")
assert_eq "$MARKER_COUNT" "1" "only one arbor block after update"
assert_contains "$(cat "$FAKE/.claude/CLAUDE.md")" "My custom rules" "user content still preserved after update"

# ============================================================
echo "=== Test 6: Uninstall removes only arbor additions ==="
FAKE="$TMPROOT/t6"
mkdir -p "$FAKE/.claude"
cat > "$FAKE/.claude/settings.json" << 'EOF'
{
  "skipDangerousModePermissionPrompt": true,
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "echo logging"}]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [{"type": "command", "command": "prettier --write"}]
      }
    ]
  }
}
EOF
cat > "$FAKE/.claude/CLAUDE.md" << 'EOF'
# My instructions

Custom user rules here.
EOF
run_install "$FAKE"
run_uninstall "$FAKE"

assert_eq "$(jq -r '.skipDangerousModePermissionPrompt' "$FAKE/.claude/settings.json")" "true" "top-level settings preserved after uninstall"
assert_eq "$(jq '.hooks.PreToolUse | length' "$FAKE/.claude/settings.json")" "1" "only Bash hook remains"
assert_eq "$(jq -r '.hooks.PreToolUse[0].matcher' "$FAKE/.claude/settings.json")" "Bash" "Bash hook preserved"
assert_eq "$(jq '.hooks.PostToolUse | length' "$FAKE/.claude/settings.json")" "1" "PostToolUse preserved after uninstall"

CONTENT=$(cat "$FAKE/.claude/CLAUDE.md")
assert_not_contains "$CONTENT" "<!-- arbor:start -->" "arbor start marker removed"
assert_not_contains "$CONTENT" "<!-- arbor:end -->" "arbor end marker removed"
assert_not_contains "$CONTENT" "mcp__arbor__" "arbor tool references removed"
assert_contains "$CONTENT" "Custom user rules here" "user CLAUDE.md content preserved"

# ============================================================
echo "=== Test 8: Uninstall cleans up empty hooks object ==="
FAKE="$TMPROOT/t8"
mkdir -p "$FAKE/.claude"
echo '{}' > "$FAKE/.claude/settings.json"
run_install "$FAKE"
run_uninstall "$FAKE"

assert_eq "$(jq 'has("hooks")' "$FAKE/.claude/settings.json")" "false" "empty hooks object removed"

# ============================================================
echo "=== Test 9: Uninstall removes CLAUDE.md if only arbor content ==="
FAKE="$TMPROOT/t9"
mkdir -p "$FAKE"
run_install "$FAKE"
run_uninstall "$FAKE"

assert_file_not_exists "$FAKE/.claude/CLAUDE.md" "empty CLAUDE.md removed"

# ============================================================
echo "=== Test 10: Uninstall preserves CLAUDE.md with user content ==="
FAKE="$TMPROOT/t10"
mkdir -p "$FAKE/.claude"
cat > "$FAKE/.claude/CLAUDE.md" << 'EOF'
# My instructions

Important stuff here.
EOF
run_install "$FAKE"
run_uninstall "$FAKE"

assert_file_exists "$FAKE/.claude/CLAUDE.md" "CLAUDE.md preserved when has user content"
CONTENT=$(cat "$FAKE/.claude/CLAUDE.md")
assert_contains "$CONTENT" "Important stuff here" "user content preserved after uninstall"
assert_not_contains "$CONTENT" "arbor" "no arbor references remain"

# ============================================================
echo "=== Test 11: Uninstall on clean system (nothing to remove) ==="
FAKE="$TMPROOT/t11"
mkdir -p "$FAKE/.claude"
echo '{"model": "opus"}' > "$FAKE/.claude/settings.json"
run_uninstall "$FAKE"

assert_eq "$(jq -r '.model' "$FAKE/.claude/settings.json")" "opus" "unrelated settings untouched"

# ============================================================
echo "=== Test 12: Install with empty settings.json ==="
FAKE="$TMPROOT/t12"
mkdir -p "$FAKE/.claude"
echo '{}' > "$FAKE/.claude/settings.json"
run_install "$FAKE"

assert_eq "$(jq -r '.hooks.PreToolUse[0].matcher' "$FAKE/.claude/settings.json")" "Grep|Glob" "hook added to empty settings"
KEYS=$(jq -r 'keys[]' "$FAKE/.claude/settings.json")
assert_contains "$KEYS" "hooks" "hooks key present"

# ============================================================
echo ""
echo "Results: $PASS passed, $FAIL failed"
if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
