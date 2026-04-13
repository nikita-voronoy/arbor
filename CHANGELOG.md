# Changelog

## [0.1.6](https://github.com/nikita-voronoy/arbor/compare/arbor-v0.1.5...arbor-v0.1.6) (2026-04-13)


### Features

* **mcp:** add `source` tool — show symbol source code with line numbers ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* **mcp:** add `callers` tool — find all functions that call a given symbol ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* **mcp:** add `summary` tool — rich single-file overview with symbols and call edges ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* **mcp:** add `symbols` tool — list all symbols of a given kind with public filter ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* **mcp:** add `implementations` tool — find all types implementing a trait ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* **mcp:** add signature search (`sig: true` parameter on `search` tool) ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* **mcp:** relative paths in all tool output (saves ~40 chars per line) ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* **analyzers:** extract `Implements` edges for Rust, Java, C#, Kotlin ([#29](https://github.com/nikita-voronoy/arbor/issues/29))


### Bug Fixes

* **analyzers:** impl names show implementing type, not trait name ([#29](https://github.com/nikita-voronoy/arbor/issues/29))


### Refactoring

* derive `Copy` for `EdgeKind`, eliminate unnecessary clones ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* replace `direction: Option<String>` with `DependencyDirection` enum ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* restrict `Palace` fields to `pub(crate)`, add public accessors ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* replace wildcard matches on own enums with exhaustive patterns ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* extract `walk_files_by_extension` helper to deduplicate analyzers ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* replace `anyhow` with `thiserror` in arbor-core ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* add `NodeKind::label()`/`short_tag()` + `Display` impl ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* decompose `ArborServer::new()` into `full_index` + `incremental_update` ([#29](https://github.com/nikita-voronoy/arbor/issues/29))


### Documentation

* update README with 14 MCP tools (was 9) ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* add `sync-tools.sh` for auto-generating README tool table from code ([#29](https://github.com/nikita-voronoy/arbor/issues/29))
* add Code of Conduct and Security Policy ([#27](https://github.com/nikita-voronoy/arbor/issues/27)) ([c394685](https://github.com/nikita-voronoy/arbor/commit/c39468500d7ff6fa94551f2058182f9da2d69124))

## [0.1.5](https://github.com/nikita-voronoy/arbor/compare/arbor-v0.1.4...arbor-v0.1.5) (2026-04-13)


### Bug Fixes

* improve monorepo detection and skip minified/build artifacts ([#23](https://github.com/nikita-voronoy/arbor/issues/23)) ([829745e](https://github.com/nikita-voronoy/arbor/commit/829745e9d91e69cecfad10e1bb7c7d8b98120aa4))
* monorepo detection, minified exclusion, CI false-positive ([#26](https://github.com/nikita-voronoy/arbor/issues/26)) ([871d950](https://github.com/nikita-voronoy/arbor/commit/871d9500c3e36fbd9afb8dd53991d42a86e6bd94))

## [0.1.4](https://github.com/nikita-voronoy/arbor/compare/arbor-v0.1.3...arbor-v0.1.4) (2026-04-13)


### Features

* add Java language support ([#18](https://github.com/nikita-voronoy/arbor/issues/18)) ([90ac317](https://github.com/nikita-voronoy/arbor/commit/90ac31799f9ce34ebd188984b5dcf4660b182053))


### Bug Fixes

* **ci:** use create-pull-request in sync-languages workflow ([#19](https://github.com/nikita-voronoy/arbor/issues/19)) ([3771447](https://github.com/nikita-voronoy/arbor/commit/3771447e42c531d4a49a225f4d7bbd36d36bec72))
* improve language analysis, remove unwrap, fix 10 code quality issues ([#20](https://github.com/nikita-voronoy/arbor/issues/20)) ([7f180a7](https://github.com/nikita-voronoy/arbor/commit/7f180a7cbf61b6046739d5c17820a9b84bdb8e33))


### Documentation

* add configuration examples to README ([#16](https://github.com/nikita-voronoy/arbor/issues/16)) ([32c284f](https://github.com/nikita-voronoy/arbor/commit/32c284fb87117ca6a27797b2a4c28eb0a876e4d7))
* sync language table from code ([#21](https://github.com/nikita-voronoy/arbor/issues/21)) ([5cb348e](https://github.com/nikita-voronoy/arbor/commit/5cb348e23895c29ed096207074ef61b2b0b7e324))

## [0.1.3](https://github.com/nikita-voronoy/arbor/compare/arbor-v0.1.2...arbor-v0.1.3) (2026-04-13)


### Features

* add Kotlin language support ([#13](https://github.com/nikita-voronoy/arbor/issues/13)) ([db79575](https://github.com/nikita-voronoy/arbor/commit/db795756382cb68e940daa5b33b047745bd23a4b))
* installer auto-configures arbor MCP hooks and instructions ([#11](https://github.com/nikita-voronoy/arbor/issues/11)) ([bc6e895](https://github.com/nikita-voronoy/arbor/commit/bc6e8955451d1113532d062fe3277179f3c309c5))


### Performance

* optimize query paths, upgrade deps to edition 2024 ([#6](https://github.com/nikita-voronoy/arbor/issues/6)) ([ffc4ae6](https://github.com/nikita-voronoy/arbor/commit/ffc4ae658a5e5bfbee489164fcd1acc8830f9091))


### Documentation

* add C# to supported languages table ([92534b3](https://github.com/nikita-voronoy/arbor/commit/92534b32def8d28c0460c6fe387bc0899c265d4f))
* add MCP vs grep/Read token efficiency benchmarks ([#7](https://github.com/nikita-voronoy/arbor/issues/7)) ([65dd9a1](https://github.com/nikita-voronoy/arbor/commit/65dd9a12c433d59c523a2be5c1db124519c88239))
* move Buy Me A Coffee button to top of README ([#12](https://github.com/nikita-voronoy/arbor/issues/12)) ([0181a73](https://github.com/nikita-voronoy/arbor/commit/0181a731eb53ff05e2b92cfc102ca320c8eafe2d))
* redesign README with logo, demo GIF, and highlights ([#10](https://github.com/nikita-voronoy/arbor/issues/10)) ([b4c6ac4](https://github.com/nikita-voronoy/arbor/commit/b4c6ac442eaf8778ee2d7b6a96a27ceb1b52399a))
* replace ASCII diagrams with Mermaid, add dotnet/runtime benchmark ([71afad4](https://github.com/nikita-voronoy/arbor/commit/71afad4987beb921c0d3eed6015efd3ce331910a))

## [0.1.2](https://github.com/nikita-voronoy/arbor/compare/arbor-v0.1.1...arbor-v0.1.2) (2026-04-12)


### Features

* add C# language support ([#4](https://github.com/nikita-voronoy/arbor/issues/4)) ([88bf269](https://github.com/nikita-voronoy/arbor/commit/88bf26925ec529493ce0151f38d645ac041412bf))
* arbor — code navigation MCP server ([666b388](https://github.com/nikita-voronoy/arbor/commit/666b3881db63e871e9a84fcc0541c01df88ae8dc))

## [0.1.1](https://github.com/nikita-voronoy/arbor/compare/arbor-v0.1.0...arbor-v0.1.1) (2026-04-12)


### Bug Fixes

* **ci:** resolve clippy errors and release-please workspace config ([7d04b96](https://github.com/nikita-voronoy/arbor/commit/7d04b9663d1111cbda17a749418793b8b9c4bbfe))


### Documentation

* add CONTRIBUTING.md with commit convention and release process ([f500f4b](https://github.com/nikita-voronoy/arbor/commit/f500f4b85ab9f5daa7bc2269001053c3a3a1757d))
