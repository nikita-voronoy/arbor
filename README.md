<p align="center">
  <img src="https://img.shields.io/badge/languages-9-blue?style=flat-square" alt="Languages">
  <img src="https://img.shields.io/badge/tree--sitter-powered-green?style=flat-square" alt="tree-sitter">
  <img src="https://img.shields.io/badge/MCP-compatible-purple?style=flat-square" alt="MCP">
  <img src="https://img.shields.io/badge/license-MIT-lightgrey?style=flat-square" alt="License">

</p>

<p align="center">
  <a href="https://buymeacoffee.com/nick_voronoy"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-blue.png" alt="Buy Me A Coffee" height="48"></a>
</p>

<h1 align="center">arbor</h1>

<p align="center">
  <strong>Code navigation MCP server that fits your entire codebase into an LLM's context.</strong><br>
  Builds a symbol graph with tree-sitter. Serves it over MCP. Compresses 1M LOC into ~500 lines.
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> &bull;
  <a href="#how-it-works">How It Works</a> &bull;
  <a href="#mcp-tools">Tools</a> &bull;
  <a href="#supported-languages">Languages</a> &bull;
  <a href="#architecture">Architecture</a>
</p>

---

## Why

LLMs are great at code — if they can see it. But context windows are finite, and `grep` dumps too much noise.

**arbor** indexes your project into a structured symbol graph, then lets the LLM navigate it surgically:

```
bevy (game engine) — 1,756 files, 21,863 functions, ~1.1M LOC
  → boot screen:     16 lines  (~400 tokens)
  → compact skeleton: 552 lines (~9k tokens)
  → indexed in:       9.5 seconds
```

The LLM sees the architecture first, then drills into exactly what it needs.

## Quick Start

One command — installs arbor and connects it to Claude Code:

**macOS / Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/nikita-voronoy/arbor/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/nikita-voronoy/arbor/main/install.ps1 | iex
```

<details>
<summary>Manual install</summary>

```bash
# Build from source
cargo install --git https://github.com/nikita-voronoy/arbor.git arbor-mcp

# Add to Claude Code
claude mcp add arbor -- arbor
```

</details>

That's it. Claude will now call `boot` → `compact` → `search` → `references` as needed.

### CLI mode

```bash
# Architecture overview
arbor /path/to/project --cli

# Token-optimized skeleton
arbor /path/to/project --compact
```

## How It Works

```
 Source files           Symbol graph          LLM context
┌──────────────┐       ┌──────────────┐      ┌────────────┐
│ .rs .py .ts  │       │  functions   │      │ boot ~150t │
│ .go .c .cpp  ├──────▶│  structs     ├─────▶│ compact    │
│ .tf .yml .sql│ parse │  traits      │ MCP  │ search     │
└──────────────┘       │  edges       │tools │ references │
                       └──────┬───────┘      │ impact     │
                              │              └────────────┘
                              ▼
                       .arbor/index.bin
                        (incremental)
```

1. **Index** — tree-sitter parses every source file into AST nodes. Arbor extracts functions, structs, traits, enums, calls, imports, type references.
2. **Persist** — the graph is saved to `.arbor/`. On restart, only changed files are re-analyzed (via xxh3 content hashing).
3. **Serve** — 9 MCP tools let the LLM explore the graph at any granularity.
4. **Two-pass resolution** — call edges that cross file boundaries are resolved after all files are indexed.

## MCP Tools

| Tool | What it does | Tokens |
|------|-------------|--------|
| `boot` | Architecture overview: modules, key types, hub functions, edge stats | ~150–400 |
| `skeleton` | Full symbol tree with signatures, organized by file | ~2k–20k |
| `compact` | Token-optimized skeleton: one-line sigs, no tests, collapsed enums | ~500–9k |
| `search` | Fuzzy symbol search, ranked: exact → prefix → contains | varies |
| `references` | All refs to a symbol: definitions, calls, imports, type refs, impls | varies |
| `dependencies` | What does this symbol depend on? (transitive, configurable depth) | varies |
| `impact` | What breaks if this symbol changes? (reverse dependency traversal) | varies |
| `tunnels` | Cross-project shared types in multi-repo mode | varies |
| `reindex` | Full re-index from scratch | — |

## Supported Languages

| Language | Functions | Structs/Classes | Traits/Interfaces | Enums | Calls | Imports |
|----------|:---------:|:---------------:|:-----------------:|:-----:|:-----:|:-------:|
| **Rust** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Python** | ✓ | ✓ (classes) | — | — | ✓ | ✓ |
| **TypeScript** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| **JavaScript** | ✓ | ✓ | — | — | ✓ | ✓ |
| **Go** | ✓ | ��� | — | — | ✓ | ✓ |
| **C** | ✓ | ✓ | — | ✓ | ✓ | ✓ |
| **C++** | ✓ | ✓ | — | ✓ | ✓ | ✓ |
| **C#** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

Plus non-code formats:

| Format | What it indexes |
|--------|----------------|
| **Ansible** | roles, tasks, handlers, variables, templates, playbooks |
| **Terraform** | resources, variables, outputs, modules, data sources |
| **SQL** | tables, columns, foreign keys |
| **Protobuf** | messages, services, RPCs |
| **OpenAPI** | endpoints, schemas |
| **Markdown** | documents, sections, links |

## Architecture

```
crates/
  arbor-core/       Graph types (Node, Palace, EdgeKind), query engine,
                    skeleton/boot/compact output formatters

  arbor-detect/     Project facet detection — scans for Cargo.toml,
                    package.json, go.mod, Makefile, etc.

  arbor-analyzers/  tree-sitter parsing for 8 languages + regex-based
                    analyzers for Ansible, Terraform, SQL, Protobuf, Markdown

  arbor-persist/    Disk persistence (bincode), incremental file hashing
                    (xxh3), file watcher (notify + ignore crate)

  arbor-mcp/        MCP server (rmcp over stdio), CLI entry point,
                    9 tool handlers
```

## Performance

Tested on real-world projects (M-series Mac):

| Project | Files | Functions | LOC | Index time | Compact size |
|---------|------:|----------:|----:|:----------:|:------------:|
| arbor (itself) | 57 | 244 | 12k | 0.4s | 141 lines |
| tokio | 776 | 6,901 | 314k | 2.9s | 623 lines |
| bevy | 1,756 | 21,863 | 1.1M | 9.5s | 552 lines |

Incremental re-index (only changed files) is typically <100ms.

## License

MIT — see [LICENSE](LICENSE).

---

<p align="center">
  Built with <a href="https://tree-sitter.github.io/">tree-sitter</a> and <a href="https://modelcontextprotocol.io/">MCP</a>
</p>
