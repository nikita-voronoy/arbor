#!/usr/bin/env bash
# test_sync_tools.sh — Verify sync-tools.sh works correctly.
# Run: ./scripts/test_sync_tools.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOLS_RS="$ROOT/crates/arbor-mcp/src/tools.rs"
README="$ROOT/README.md"
ERRORS=0

fail() { echo "FAIL: $1"; ERRORS=$((ERRORS + 1)); }

# ---------- 1. Count #[tool()] attributes in source ----------

EXPECTED_COUNT=$(grep -c '#\[tool(' "$TOOLS_RS")
echo "Expected tool count from source: $EXPECTED_COUNT"

# ---------- 2. Run sync-tools and check output ----------

OUTPUT=$(bash "$ROOT/scripts/sync-tools.sh" 2>&1)
SYNCED_COUNT=$(echo "$OUTPUT" | grep -o '[0-9]\+')

if [ "$SYNCED_COUNT" -ne "$EXPECTED_COUNT" ]; then
    fail "sync-tools reported $SYNCED_COUNT tools, expected $EXPECTED_COUNT"
fi

# ---------- 3. Verify README has correct count in badge ----------

BADGE_COUNT=$(grep -o 'MCP_tools-[0-9]*' "$README" | grep -o '[0-9]*')
if [ "$BADGE_COUNT" -ne "$EXPECTED_COUNT" ]; then
    fail "README badge shows $BADGE_COUNT, expected $EXPECTED_COUNT"
fi

# ---------- 4. Verify README table has correct number of rows ----------

TABLE_ROWS=$(sed -n '/TOOLS_TABLE:START/,/TOOLS_TABLE:END/p' "$README" | grep -c '| \*\*`')
if [ "$TABLE_ROWS" -ne "$EXPECTED_COUNT" ]; then
    fail "README table has $TABLE_ROWS rows, expected $EXPECTED_COUNT"
fi

# ---------- 5. Verify idempotency ----------

cp "$README" /tmp/readme_idempotent.md
bash "$ROOT/scripts/sync-tools.sh" >/dev/null 2>&1
if ! diff -q "$README" /tmp/readme_idempotent.md >/dev/null 2>&1; then
    fail "sync-tools.sh is NOT idempotent — second run changed README"
fi
rm -f /tmp/readme_idempotent.md

# ---------- 6. Verify every tool name from source appears in table ----------

while IFS= read -r line; do
    if [[ "$line" =~ name[[:space:]]*=[[:space:]]*\"([^\"]+)\" ]]; then
        TOOL_NAME="${BASH_REMATCH[1]}"
        if ! grep -q "\`$TOOL_NAME\`" "$README"; then
            fail "Tool '$TOOL_NAME' not found in README table"
        fi
    fi
done < <(grep 'name *= *"' "$TOOLS_RS")

# ---------- Result ----------

if [ "$ERRORS" -gt 0 ]; then
    echo "FAILED: $ERRORS error(s)"
    exit 1
fi

echo "OK: all checks passed ($EXPECTED_COUNT tools synced)"
