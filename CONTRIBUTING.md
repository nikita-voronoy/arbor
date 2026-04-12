# Contributing to arbor

## Development Setup

```bash
git clone git@github.com:nikita-voronoy/arbor.git
cd arbor
cargo test --all
```

## Commit Convention

This project uses [Conventional Commits](https://www.conventionalcommits.org/) for automated versioning and changelog generation.

### Format

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

### Types

| Type | Purpose | Triggers release? |
|------|---------|:-----------------:|
| `feat` | New feature | yes (minor) |
| `fix` | Bug fix | yes (patch) |
| `perf` | Performance improvement | yes (patch) |
| `refactor` | Code restructuring (no behavior change) | yes (patch) |
| `docs` | Documentation only | yes (patch) |
| `test` | Adding/fixing tests | no |
| `ci` | CI/CD changes | no |
| `chore` | Maintenance (deps, configs) | no |
| `build` | Build system changes | no |
| `revert` | Revert a previous commit | depends |

### Breaking Changes

Add `!` after the type or include `BREAKING CHANGE:` in the footer:

```
feat!: remove support for Python 2

BREAKING CHANGE: Python 2 is no longer supported.
```

Breaking changes bump the **major** version (after v1.0.0).

### Scopes (optional)

Use the crate name as scope when the change is localized:

```
feat(core): add method signature compression
fix(analyzers): handle empty Go files
perf(persist): switch to zstd compression
```

### Examples

```
feat: add Java language support
fix: handle files with BOM marker
perf: cache tree-sitter parsers across files
docs: add performance benchmarks to README
refactor(core): split skeleton.rs into boot and compact modules
test: add integration tests for mixed Rust+Terraform projects
ci: add aarch64-linux build target
chore: update tree-sitter-rust to 0.25
```

## Pull Requests

1. PR titles **must** follow conventional commit format — CI enforces this
2. release-please uses PR titles for the changelog, so make them descriptive
3. One logical change per PR

## How Releases Work

```
Push to main
    │
    ├── CI runs (test, clippy, fmt)
    │
    └── release-please analyzes new commits
        │
        ├── No releasable commits (ci:, chore:, test:)
        │   └── nothing happens
        │
        └── Has feat:/fix:/perf: commits
            └── Opens/updates a "Release PR"
                │
                └── Merge the Release PR
                    │
                    ├── Version bumped in all Cargo.toml files
                    ├── CHANGELOG.md updated
                    ├── Git tag created (v0.2.0)
                    └── Binaries built and attached to GitHub Release
                        ├── arbor-x86_64-unknown-linux-gnu.tar.gz
                        ├── arbor-aarch64-unknown-linux-gnu.tar.gz
                        ├── arbor-x86_64-apple-darwin.tar.gz
                        ├── arbor-aarch64-apple-darwin.tar.gz
                        └── arbor-x86_64-pc-windows-msvc.zip
```

You never manually bump versions, write changelogs, or create tags.

## Version Bumping Rules

| Commit type | Before v1.0 | After v1.0 |
|-------------|:-----------:|:----------:|
| `fix:` | 0.1.0 → 0.1.1 | 1.0.0 → 1.0.1 |
| `feat:` | 0.1.0 → 0.2.0 | 1.0.0 → 1.1.0 |
| `feat!:` / `BREAKING CHANGE` | 0.1.0 → 0.2.0 | 1.0.0 → 2.0.0 |

Before v1.0, breaking changes bump minor instead of major (per semver spec).

## Project Structure

```
crates/
  arbor-core/        Graph types, query engine, skeleton/boot formatters
  arbor-detect/      Project type detection (Cargo.toml → Rust, etc.)
  arbor-analyzers/   tree-sitter parsing + IaC/docs/schema analyzers
  arbor-persist/     Disk persistence, file hashing, file watcher
  arbor-mcp/         MCP server, CLI entry point, tool handlers
tests/
  fixtures/          Test projects: Rust, Python, TS, Go, C, Ansible, TF, etc.
```

## Running Tests

```bash
cargo test --all          # all 70 tests
cargo test -p arbor-analyzers  # just analyzer tests
cargo clippy --all        # lint
cargo fmt --all           # format
```
