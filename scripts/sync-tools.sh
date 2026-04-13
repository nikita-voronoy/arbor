#!/usr/bin/env bash
# sync-tools.sh — Generate MCP tools table and counts in README + CONTRIBUTING from code.
# Run: ./scripts/sync-tools.sh
# CI: .github/workflows/sync-tools.yml
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOLS_RS="$ROOT/crates/arbor-mcp/src/tools.rs"
README="$ROOT/README.md"
CONTRIBUTING="$ROOT/CONTRIBUTING.md"

# ---------- 1. Extract tools from #[tool(...)] attributes ----------

TOOLS_FILE=$(mktemp)
trap 'rm -f "$TOOLS_FILE" "$README.tmp"' EXIT

name=""
desc=""
in_tool=0

while IFS= read -r line; do
    if [[ "$line" =~ \#\[tool\( ]]; then
        in_tool=1; name=""; desc=""
    fi
    if (( in_tool )); then
        if [[ "$line" =~ name[[:space:]]*=[[:space:]]*\"([^\"]+)\" ]]; then
            name="${BASH_REMATCH[1]}"
        fi
        if [[ "$line" =~ description[[:space:]]*=[[:space:]]*\"([^\"]+)\" ]]; then
            desc="${BASH_REMATCH[1]}"
        fi
        if [[ "$line" =~ \)\] ]]; then
            if [[ -n "$name" && -n "$desc" ]]; then
                echo "${name}	${desc}" >> "$TOOLS_FILE"
            fi
            in_tool=0
        fi
    fi
done < "$TOOLS_RS"

TOOL_COUNT=$(wc -l < "$TOOLS_FILE" | tr -d ' ')

# ---------- 2. Generate markdown table file ----------

TABLE_FILE=$(mktemp)
trap 'rm -f "$TOOLS_FILE" "$TABLE_FILE" "$README.tmp"' EXIT

echo "| Tool | What it does |" > "$TABLE_FILE"
echo "|------|-------------|" >> "$TABLE_FILE"

while IFS=$'\t' read -r tname tdesc; do
    [[ -z "$tname" ]] && continue
    echo "| **\`${tname}\`** | ${tdesc} |" >> "$TABLE_FILE"
done < "$TOOLS_FILE"

# ---------- 3. Update README: tools table (replace between markers) ----------

awk '
    /<!-- TOOLS_TABLE:START -->/ { print; system("cat '"$TABLE_FILE"'"); skip=1; next }
    /<!-- TOOLS_TABLE:END -->/   { skip=0 }
    !skip { print }
' "$README" > "$README.tmp" && mv "$README.tmp" "$README"

# ---------- 4. Update README: tool count badge ----------

sed -i.bak "s|<!-- TOOLS_BADGE:START -->.*<!-- TOOLS_BADGE:END -->|<!-- TOOLS_BADGE:START --><img src=\"https://img.shields.io/badge/MCP_tools-${TOOL_COUNT}-purple?style=flat-square\" alt=\"MCP\"><!-- TOOLS_BADGE:END -->|" "$README"
rm -f "$README.bak"

# ---------- 5. Update README: inline counts ----------

sed -i.bak "s|[0-9]\{1,\} surgical MCP tools|${TOOL_COUNT} surgical MCP tools|" "$README"
rm -f "$README.bak"

sed -i.bak "s|[0-9]\{1,\} tool handlers|${TOOL_COUNT} tool handlers|" "$README"
rm -f "$README.bak"

sed -i.bak "s|[0-9]\{1,\} MCP tools let the LLM|${TOOL_COUNT} MCP tools let the LLM|" "$README"
rm -f "$README.bak"

# ---------- 6. Update CONTRIBUTING: test count ----------

if command -v cargo &>/dev/null; then
    TEST_OUTPUT=$(cargo test --all 2>&1 || true)
    TEST_COUNT=$(echo "$TEST_OUTPUT" | grep -o '[0-9]* passed' | awk '{s+=$1} END {print s+0}')
    if [ "$TEST_COUNT" -gt 0 ] 2>/dev/null; then
        sed -i.bak "s|all [0-9]\{1,\} tests|all ${TEST_COUNT} tests|" "$CONTRIBUTING"
        rm -f "$CONTRIBUTING.bak"
    fi
fi

echo "Synced: ${TOOL_COUNT} tools"
