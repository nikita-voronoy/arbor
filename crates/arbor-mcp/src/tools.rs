use arbor_analyzers::AnalyzerRegistry;
use arbor_core::NodeIndex;
use arbor_core::palace::Palace;
use parking_lot::RwLock;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::path::{Path, PathBuf};

pub struct ArborServer {
    palace: RwLock<Palace>,
    root: PathBuf,
    facets: Vec<String>,
    tool_router: ToolRouter<Self>,
}

impl ArborServer {
    pub fn new(root: PathBuf) -> anyhow::Result<Self> {
        let registry = AnalyzerRegistry::new()?;
        let facets = arbor_detect::detect(&root);
        let facet_labels: Vec<String> = facets.iter().map(|f| f.label().to_string()).collect();

        let cached_palace = arbor_persist::store::load(&root).unwrap_or(None);
        let mut palace = cached_palace.unwrap_or_default();

        let changed = if palace.node_count() == 0 {
            Self::full_index(&root, &registry, &mut palace)?
        } else {
            Self::incremental_update(&root, &registry, &facets, &mut palace)?
        };

        arbor_persist::store::save(&palace, &root)?;

        if changed > 0 {
            eprintln!("Arbor: incrementally updated {changed} files");
        }

        Ok(Self {
            palace: RwLock::new(palace),
            root,
            facets: facet_labels,
            tool_router: Self::tool_router(),
        })
    }

    /// Full index from scratch — no hashing overhead during analysis.
    fn full_index(
        root: &Path,
        registry: &AnalyzerRegistry,
        palace: &mut Palace,
    ) -> anyhow::Result<usize> {
        registry.analyze_project(root, palace)?;
        palace.resolve_pending_calls();

        let mut hashes = arbor_persist::hasher::FileHashes::new();
        for path in palace.file_paths() {
            let _ = hashes.check_file(path);
        }
        hashes.save(root)?;
        Ok(0)
    }

    /// Incremental update — only re-analyze changed/new/deleted files.
    fn incremental_update(
        root: &Path,
        registry: &AnalyzerRegistry,
        facets: &[arbor_detect::ProjectFacet],
        palace: &mut Palace,
    ) -> anyhow::Result<usize> {
        let mut hashes = arbor_persist::hasher::FileHashes::load(root).unwrap_or_default();

        let current_files: std::collections::HashSet<PathBuf> =
            palace.file_paths().map(Path::to_path_buf).collect();

        // Remove deleted files
        let tracked: Vec<PathBuf> = hashes
            .tracked_files()
            .map(std::path::Path::to_path_buf)
            .collect();
        for path in &tracked {
            if !path.exists() {
                palace.remove_file(path);
                hashes.remove_file(path);
            }
        }

        // Collect modified files
        let mut changed_files = Vec::new();
        for path in &current_files {
            if let Ok(
                arbor_persist::hasher::FileStatus::New
                | arbor_persist::hasher::FileStatus::Modified,
            ) = hashes.check_file(path)
            {
                palace.remove_file(path);
                changed_files.push(path.clone());
            }
        }

        // Collect newly created files (not yet in cache)
        let all_files = arbor_persist::watcher::walk_files(root);
        for path in &all_files {
            if !current_files.contains(path)
                && matches!(
                    hashes.check_file(path),
                    Ok(arbor_persist::hasher::FileStatus::New)
                )
            {
                changed_files.push(path.clone());
            }
        }

        let count = changed_files.len();
        let mut errors = 0usize;
        for path in &changed_files {
            match std::fs::read_to_string(path) {
                Ok(source) => {
                    for analyzer in registry.for_facets(facets) {
                        if let Err(e) = analyzer.analyze_file(path, &source, palace) {
                            eprintln!("Arbor: failed to analyze {}: {e}", path.display());
                            errors += 1;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Arbor: failed to read {}: {e}", path.display());
                    errors += 1;
                }
            }
        }
        if errors > 0 {
            eprintln!("Arbor: {errors} file(s) had errors during incremental update");
        }

        palace.resolve_pending_calls();
        hashes.save(root)?;
        Ok(count)
    }

    fn project_name(&self) -> &str {
        self.root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    }

    /// Read the source code of a symbol using its span info.
    fn read_symbol_source(
        palace: &Palace,
        root: &Path,
        idx: NodeIndex,
        max_lines: usize,
    ) -> String {
        let Some(node) = palace.get_node(idx) else {
            return "Node not found in graph".to_string();
        };

        let content = match std::fs::read_to_string(&node.file) {
            Ok(c) => c,
            Err(e) => return format!("Failed to read {}: {e}", node.file.display()),
        };

        let lines: Vec<&str> = content.lines().collect();
        let start = node.span.start_line.saturating_sub(1) as usize;
        let end = (node.span.end_line as usize).min(lines.len());

        if start >= lines.len() {
            return format!(
                "Span {}:{} is out of range for {} ({} lines)",
                node.span.start_line,
                node.span.end_line,
                node.file.display(),
                lines.len()
            );
        }

        let rel = node.file.strip_prefix(root).unwrap_or(&node.file);
        let sig = node.signature.as_deref().unwrap_or(&node.name);
        let mut out = format!(
            "// {} {} ({}:{}–{})\n",
            node.kind.label(),
            sig,
            rel.display(),
            node.span.start_line,
            node.span.end_line
        );

        let source_lines = &lines[start..end];
        if source_lines.len() > max_lines {
            for (i, line) in source_lines.iter().take(max_lines).enumerate() {
                let _ = writeln!(out, "{:>4} | {line}", start + i + 1);
            }
            let _ = writeln!(
                out,
                "  ... truncated ({} more lines)",
                source_lines.len() - max_lines
            );
        } else {
            for (i, line) in source_lines.iter().enumerate() {
                let _ = writeln!(out, "{:>4} | {line}", start + i + 1);
            }
        }
        out
    }

    /// Format a rich summary of a single file.
    fn format_file_summary(palace: &Palace, path: &Path, indices: &[NodeIndex]) -> String {
        use arbor_core::graph::NodeKind;

        let mut out = format!("File: {}\n", path.display());

        let mut symbols: Vec<_> = indices
            .iter()
            .filter_map(|&idx| palace.get_node(idx).map(|n| (idx, n)))
            .filter(|(_, n)| !matches!(n.kind, NodeKind::File))
            .collect();
        symbols.sort_by_key(|(_, n)| n.span.start_line);

        if symbols.is_empty() {
            out.push_str("  (no symbols found)\n");
            return out;
        }

        let _ = writeln!(out, "{} symbols:\n", symbols.len());

        for (idx, node) in &symbols {
            let vis = if node.visibility == arbor_core::graph::Visibility::Public {
                "pub "
            } else {
                ""
            };
            let sig = node.signature.as_deref().unwrap_or(&node.name);
            let _ = writeln!(
                out,
                "  L{:<4} {vis}{} {}",
                node.span.start_line,
                node.kind.label(),
                sig
            );

            // Show calls from this symbol
            let mut calls = palace.callees(*idx);
            if !calls.is_empty() {
                calls.sort_unstable();
                calls.dedup();
                let _ = writeln!(out, "         → calls: [{}]", calls.join(", "));
            }
        }
        out
    }

    /// CLI helpers (not MCP tools)
    pub fn boot_cli(&self) -> String {
        let palace = self.palace.read();
        palace.boot(self.project_name(), &self.facets.join("+"))
    }

    pub fn skeleton_cli(&self) -> String {
        let palace = self.palace.read();
        palace.skeleton(None, 3)
    }

    pub fn compact_cli(&self) -> String {
        let palace = self.palace.read();
        palace.compact_skeleton(None, 500, true)
    }
}

// --- Parameter structs ---

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SkeletonParams {
    /// Optional path prefix to filter (e.g. 'src/auth')
    pub path: Option<String>,
    /// Max nesting depth (default 3)
    pub depth: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SymbolParams {
    /// Symbol name to search for
    pub symbol: String,
}

/// Direction for dependency traversal
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DependencyDirection {
    /// What this symbol depends on
    #[default]
    Outgoing,
    /// What depends on this symbol
    Incoming,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DependenciesParams {
    /// Symbol name
    pub symbol: String,
    /// Direction: 'outgoing' (default) or 'incoming'
    pub direction: Option<DependencyDirection>,
    /// Max traversal depth (default 5)
    pub max_depth: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CompactParams {
    /// Optional path prefix to filter
    pub path: Option<String>,
    /// Max items to show (default 500)
    pub max_items: Option<usize>,
    /// Skip test functions (default true)
    pub skip_tests: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ImpactParams {
    /// Symbol name to analyze impact for
    pub symbol: String,
    /// Max traversal depth (default 10)
    pub max_depth: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SearchParams {
    /// Search query (substring match)
    pub query: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SourceParams {
    /// Symbol name to show source for
    pub symbol: String,
    /// Max lines to return (default 100)
    pub max_lines: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SummaryParams {
    /// File path (relative to project root) to summarize
    pub path: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SymbolsParams {
    /// Kind filter: "fn", "struct", "trait", "enum", "macro", or "all" (default "all")
    pub kind: Option<String>,
    /// Only show public symbols (default false)
    pub public_only: Option<bool>,
}

// --- Tool implementations ---

#[tool_router(router = tool_router)]
impl ArborServer {
    #[tool(
        name = "boot",
        description = "Get a compact boot screen overview of the project (~170 tokens): project type, file/function/struct counts, top-level modules, key public types. Call this first."
    )]
    async fn boot(&self) -> String {
        let palace = self.palace.read();
        palace.boot(self.project_name(), &self.facets.join("+"))
    }

    #[tool(
        name = "skeleton",
        description = "Get a compact skeleton showing all symbols (functions, structs, traits, enums) organized by file. Optionally filter by path prefix and control depth."
    )]
    async fn skeleton(&self, params: Parameters<SkeletonParams>) -> String {
        let palace = self.palace.read();
        let depth = params.0.depth.unwrap_or(3);
        params.0.path.as_ref().map_or_else(
            || palace.skeleton(None, depth),
            |p| {
                let full_path = self.root.join(p);
                palace.skeleton(Some(full_path.as_path()), depth)
            },
        )
    }

    #[tool(
        name = "compact",
        description = "Get a ultra-compact token-optimized skeleton. Uses abbreviated tags (fn/st/tr/en) and compressed signatures. Best for large codebases where full skeleton is too verbose."
    )]
    async fn compact(&self, params: Parameters<CompactParams>) -> String {
        let palace = self.palace.read();
        let max_items = params.0.max_items.unwrap_or(500);
        let skip_tests = params.0.skip_tests.unwrap_or(true);
        params.0.path.as_ref().map_or_else(
            || palace.compact_skeleton(None, max_items, skip_tests),
            |p| {
                let full_path = self.root.join(p);
                palace.compact_skeleton(Some(full_path.as_path()), max_items, skip_tests)
            },
        )
    }

    #[tool(
        name = "references",
        description = "Find all references to a symbol: definitions, calls, imports, type refs, implementations. Returns file locations and reference kinds."
    )]
    async fn references(&self, params: Parameters<SymbolParams>) -> String {
        let palace = self.palace.read();
        let refs = palace.references(&params.0.symbol);
        if refs.is_empty() {
            return format!("No references found for '{}'", params.0.symbol);
        }

        // Filter out File nodes (line 0 noise)
        let refs: Vec<_> = refs
            .into_iter()
            .filter(|r| palace.is_real_symbol(r.node))
            .collect();

        let mut out = format!(
            "References to '{}' ({} found):\n",
            params.0.symbol,
            refs.len()
        );
        for r in &refs {
            if let Some(node) = palace.get_node(r.node) {
                let _ = writeln!(
                    out,
                    "  {} in {} ({}:{})",
                    r.kind,
                    node.name,
                    node.file.display(),
                    node.span.start_line
                );
            }
        }
        out
    }

    #[tool(
        name = "dependencies",
        description = "Get transitive dependencies of a symbol. Direction 'outgoing' (default) shows what it depends on; 'incoming' shows what depends on it."
    )]
    async fn dependencies(&self, params: Parameters<DependenciesParams>) -> String {
        let palace = self.palace.read();

        // Use primary definition, not every occurrence
        let Some(node_idx) = palace.find_primary(&params.0.symbol) else {
            return format!("Symbol '{}' not found", params.0.symbol);
        };

        let max_depth = params.0.max_depth.unwrap_or(5);
        let direction = params.0.direction.unwrap_or_default();

        let deps = if direction == DependencyDirection::Incoming {
            palace.impact(node_idx, max_depth)
        } else {
            palace.dependencies(node_idx, max_depth)
        };

        let Some(node) = palace.get_node(node_idx) else {
            return format!(
                "Symbol '{}' found but node missing from graph",
                params.0.symbol
            );
        };
        let dir_label = if direction == DependencyDirection::Incoming {
            "Dependents of"
        } else {
            "Dependencies of"
        };
        let deps: Vec<_> = deps
            .into_iter()
            .filter(|(idx, _)| palace.is_real_symbol(*idx))
            .collect();

        let mut out = format!("{dir_label} '{}' ({} found):\n", node.name, deps.len());
        for (dep_idx, depth) in &deps {
            if let Some(dep) = palace.get_node(*dep_idx) {
                let _ = writeln!(
                    out,
                    "  [depth {depth}] {} {} ({}:{})",
                    dep.kind.label(),
                    dep.name,
                    dep.file.display(),
                    dep.span.start_line
                );
            }
        }
        out
    }

    #[tool(
        name = "impact",
        description = "Impact analysis: find everything that would be affected if the given symbol changes. Shows all transitive dependents."
    )]
    async fn impact(&self, params: Parameters<ImpactParams>) -> String {
        let palace = self.palace.read();

        let Some(node_idx) = palace.find_primary(&params.0.symbol) else {
            return format!("Symbol '{}' not found", params.0.symbol);
        };

        let max_depth = params.0.max_depth.unwrap_or(10);
        let impacts = palace.impact(node_idx, max_depth);

        let impacts: Vec<_> = impacts
            .into_iter()
            .filter(|(idx, _)| palace.is_real_symbol(*idx))
            .collect();

        let Some(node) = palace.get_node(node_idx) else {
            return format!(
                "Symbol '{}' found but node missing from graph",
                params.0.symbol
            );
        };
        let mut out = format!(
            "Impact of changing '{}': {} affected symbols\n",
            node.name,
            impacts.len()
        );
        for (imp_idx, depth) in &impacts {
            if let Some(imp) = palace.get_node(*imp_idx) {
                let _ = writeln!(
                    out,
                    "  [depth {depth}] {} ({}:{})",
                    imp.name,
                    imp.file.display(),
                    imp.span.start_line
                );
            }
        }
        out
    }

    #[tool(
        name = "search",
        description = "Fuzzy search for symbols (functions, structs, traits, enums) by name substring. Results ranked: exact > prefix > contains."
    )]
    async fn search(&self, params: Parameters<SearchParams>) -> String {
        let palace = self.palace.read();
        let results = palace.search(&params.0.query);
        if results.is_empty() {
            return format!("No symbols matching '{}'", params.0.query);
        }

        let mut out = format!(
            "Symbols matching '{}' ({} found):\n",
            params.0.query,
            results.len()
        );
        for idx in results.iter().take(20) {
            if let Some(node) = palace.get_node(*idx) {
                let kind = node.kind.label();
                let sig = node.signature.as_deref().unwrap_or(&node.name);
                let _ = writeln!(
                    out,
                    "  {kind} {sig} ({}:{})",
                    node.file.display(),
                    node.span.start_line
                );
            }
        }
        if results.len() > 20 {
            let _ = writeln!(out, "  ... and {} more", results.len() - 20);
        }
        out
    }

    #[tool(
        name = "source",
        description = "Show the source code of a symbol (function, struct, trait, etc.) by name. Returns the actual implementation with line numbers. Use this instead of reading whole files when you know the symbol name."
    )]
    async fn source(&self, params: Parameters<SourceParams>) -> String {
        let max_lines = params.0.max_lines.unwrap_or(100);

        let idx = self
            .palace
            .read()
            .find_primary(&params.0.symbol)
            .or_else(|| {
                self.palace
                    .read()
                    .find_by_name(&params.0.symbol)
                    .iter()
                    .copied()
                    .find(|&i| self.palace.read().is_real_symbol(i))
            });

        let Some(idx) = idx else {
            return format!("Symbol '{}' not found", params.0.symbol);
        };
        let palace = self.palace.read();
        Self::read_symbol_source(&palace, &self.root, idx, max_lines)
    }

    #[tool(
        name = "callers",
        description = "Find all functions that call a given symbol. Returns caller names with file locations. Simpler than 'references' when you just need to know who calls what."
    )]
    async fn callers(&self, params: Parameters<SymbolParams>) -> String {
        let refs = self.palace.read().references(&params.0.symbol);

        let palace = self.palace.read();
        let callers: Vec<_> = refs
            .iter()
            .filter(|r| matches!(r.kind, arbor_core::query::ReferenceKind::Call))
            .filter_map(|r| {
                palace.get_node(r.node).map(|n| {
                    (
                        n.kind.label(),
                        n.signature.as_deref().unwrap_or(&n.name).to_string(),
                        n.file.display().to_string(),
                        n.span.start_line,
                    )
                })
            })
            .collect();
        drop(palace);

        if callers.is_empty() {
            return format!("No callers found for '{}'", params.0.symbol);
        }

        let mut out = format!(
            "Callers of '{}' ({} found):\n",
            params.0.symbol,
            callers.len()
        );
        for (kind, sig, file, line) in &callers {
            let _ = writeln!(out, "  {kind} {sig} ({file}:{line})");
        }
        out
    }

    #[tool(
        name = "summary",
        description = "Get a rich summary of a single file: all symbols with signatures, visibility, and call relationships. More detailed than skeleton for a specific file."
    )]
    async fn summary(&self, params: Parameters<SummaryParams>) -> String {
        let full_path = self.root.join(&params.0.path);
        let resolved = {
            let palace = self.palace.read();
            if palace.nodes_in_file(&full_path).is_empty() {
                palace
                    .file_paths()
                    .find(|p| p.ends_with(&params.0.path))
                    .map(Path::to_path_buf)
            } else {
                Some(full_path)
            }
        };
        let Some(path) = resolved else {
            return format!("File '{}' not found in index", params.0.path);
        };

        let indices = self.palace.read().nodes_in_file(&path).to_vec();
        let palace = self.palace.read();
        Self::format_file_summary(&palace, &path, &indices)
    }

    #[tool(
        name = "symbols",
        description = "List all symbols of a given kind across the project. Kinds: fn, struct, trait, enum, macro, module, or 'all'. Useful for getting a project-wide view of types, traits, or entry points."
    )]
    async fn symbols(&self, params: Parameters<SymbolsParams>) -> String {
        use arbor_core::graph::NodeKind;

        let palace = self.palace.read();
        let public_only = params.0.public_only.unwrap_or(false);
        let kind_filter = params.0.kind.as_deref().unwrap_or("all");

        let target_kinds: Vec<NodeKind> = match kind_filter {
            "fn" | "function" => vec![NodeKind::Function],
            "struct" => vec![NodeKind::Struct],
            "trait" => vec![NodeKind::Trait],
            "enum" => vec![NodeKind::Enum],
            "macro" => vec![NodeKind::Macro],
            "mod" | "module" => vec![NodeKind::Module],
            "type" => vec![NodeKind::Struct, NodeKind::Enum, NodeKind::Trait],
            "all" => vec![
                NodeKind::Function,
                NodeKind::Struct,
                NodeKind::Trait,
                NodeKind::Enum,
                NodeKind::Macro,
            ],
            other => {
                return format!(
                    "Unknown kind '{other}'. Use: fn, struct, trait, enum, macro, module, type, all"
                );
            }
        };

        // Collect into owned data so we can drop the lock early
        let mut items: Vec<(String, String, &'static str, &'static str, u32)> = palace
            .node_weights()
            .filter(|n| target_kinds.contains(&n.kind))
            .filter(|n| !public_only || n.visibility == arbor_core::graph::Visibility::Public)
            .map(|n| {
                let rel = n.file.strip_prefix(&self.root).unwrap_or(&n.file);
                let sig = n.signature.as_deref().unwrap_or(&n.name).to_string();
                let vis = if n.visibility == arbor_core::graph::Visibility::Public {
                    "pub "
                } else {
                    ""
                };
                (
                    rel.display().to_string(),
                    sig,
                    n.kind.label(),
                    vis,
                    n.span.start_line,
                )
            })
            .collect();
        drop(palace);

        items.sort_by(|a, b| a.2.cmp(b.2).then(a.1.cmp(&b.1)));

        if items.is_empty() {
            return format!("No {kind_filter} symbols found");
        }

        let mut out = format!("{} {} symbols found:\n", items.len(), kind_filter);
        for (rel, sig, kind, vis, line) in items.iter().take(50) {
            let _ = writeln!(out, "  {vis}{kind} {sig} ({rel}:{line})");
        }
        if items.len() > 50 {
            let _ = writeln!(out, "  ... and {} more", items.len() - 50);
        }
        out
    }

    #[tool(
        name = "reindex",
        description = "Re-index the project from scratch. Use after significant file changes."
    )]
    async fn reindex(&self) -> String {
        let mut palace = self.palace.write();
        *palace = Palace::new();
        let registry = match AnalyzerRegistry::new() {
            Ok(r) => r,
            Err(e) => return format!("Failed to initialize analyzers: {e}"),
        };
        match registry.analyze_project(&self.root, &mut palace) {
            Ok(facets) => {
                if let Err(e) = arbor_persist::store::save(&palace, &self.root) {
                    eprintln!("Arbor: failed to save index: {e}");
                }
                let stats = palace.stats();
                drop(palace);
                // Rebuild file hashes (no lock needed)
                let mut hashes = arbor_persist::hasher::FileHashes::new();
                for path in arbor_persist::watcher::walk_files(&self.root) {
                    let _ = hashes.check_file(&path);
                }
                if let Err(e) = hashes.save(&self.root) {
                    eprintln!("Arbor: failed to save file hashes: {e}");
                }
                format!(
                    "Re-indexed: {} files, {} fn, {} structs, {} enums, {} traits | Facets: {}",
                    stats.files,
                    stats.functions,
                    stats.structs,
                    stats.enums,
                    stats.traits,
                    facets
                        .iter()
                        .map(arbor_detect::ProjectFacet::label)
                        .collect::<Vec<_>>()
                        .join("+")
                )
            }
            Err(e) => format!("Re-index failed: {e}"),
        }
    }

    #[tool(
        name = "tunnels",
        description = "Show cross-project tunnels: shared types and symbols that connect different wings (projects) in a multi-project palace."
    )]
    async fn tunnels(&self) -> String {
        let palace = self.palace.read();
        palace.format_tunnels()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ArborServer {}

#[cfg(test)]
mod tests {
    use super::*;
    use arbor_analyzers::AnalyzerRegistry;
    use arbor_core::graph::{NodeKind, Visibility};
    use std::path::PathBuf;

    fn arbor_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn index_arbor() -> (Palace, PathBuf) {
        let root = std::fs::canonicalize(arbor_root()).unwrap();
        let mut palace = Palace::new();
        let registry = AnalyzerRegistry::new().unwrap();
        registry.analyze_project(&root, &mut palace).unwrap();
        (palace, root)
    }

    #[test]
    fn source_shows_function_code() {
        let (palace, root) = index_arbor();
        let idx = palace.find_primary("resolve_pending_calls").unwrap();
        let output = ArborServer::read_symbol_source(&palace, &root, idx, 100);

        assert!(
            output.contains("resolve_pending_calls"),
            "should contain function name"
        );
        assert!(output.contains("pending"), "should contain function body");
        assert!(output.contains(" | "), "should have line numbers");
    }

    #[test]
    fn source_shows_struct_code() {
        let (palace, root) = index_arbor();
        let idx = palace.find_primary("Palace").unwrap();
        let output = ArborServer::read_symbol_source(&palace, &root, idx, 50);

        assert!(output.contains("Palace"), "should show struct name");
        assert!(output.contains("struct"), "header should show kind");
    }

    #[test]
    fn source_respects_max_lines() {
        let (palace, root) = index_arbor();
        let idx = palace.find_primary("resolve_pending_calls").unwrap();
        let output = ArborServer::read_symbol_source(&palace, &root, idx, 3);

        assert!(
            output.contains("truncated"),
            "should truncate long functions"
        );
    }

    #[test]
    fn callers_of_add_node() {
        let (palace, _root) = index_arbor();
        let refs = palace.references("add_node");
        let callers: Vec<_> = refs
            .iter()
            .filter(|r| matches!(r.kind, arbor_core::query::ReferenceKind::Call))
            .filter_map(|r| palace.get_node(r.node))
            .collect();

        assert!(!callers.is_empty(), "add_node should have callers");
        // walk_tree and various analyzers should call add_node
        let caller_names: Vec<&str> = callers.iter().map(|n| n.name.as_str()).collect();
        assert!(
            caller_names.iter().any(|n| n.contains("walk")
                || n.contains("parse")
                || n.contains("analyze")
                || n.contains("extract")
                || n.contains("merge")),
            "expected analyzer/walker callers, got: {caller_names:?}"
        );
    }

    #[test]
    fn summary_shows_file_symbols() {
        let (palace, root) = index_arbor();
        let path = root.join("crates/arbor-core/src/palace.rs");
        let indices = palace.nodes_in_file(&path);
        assert!(!indices.is_empty(), "palace.rs should have symbols");

        let output = ArborServer::format_file_summary(&palace, &path, indices);

        assert!(output.contains("Palace"), "should contain Palace struct");
        assert!(output.contains("add_node"), "should contain add_node fn");
        assert!(output.contains('L'), "should have line numbers");
        assert!(output.contains("calls:"), "should show call relationships");
    }

    #[test]
    fn summary_partial_path_match() {
        let (palace, root) = index_arbor();
        // Full path exists
        let full = root.join("crates/arbor-core/src/palace.rs");
        assert!(!palace.nodes_in_file(&full).is_empty());

        // Partial path should also find it via file_paths iteration
        let found = palace
            .file_paths()
            .find(|p| p.ends_with("arbor-core/src/palace.rs"));
        assert!(found.is_some(), "partial path should match");
    }

    #[test]
    fn symbols_lists_all_traits() {
        let (palace, _root) = index_arbor();
        let traits: Vec<_> = palace
            .node_weights()
            .filter(|n| n.kind == NodeKind::Trait && n.visibility == Visibility::Public)
            .collect();

        assert!(!traits.is_empty(), "should find public traits");
        assert!(
            traits.iter().any(|n| n.name == "Analyzer"),
            "should find Analyzer trait"
        );
    }

    #[test]
    fn symbols_lists_public_structs() {
        let (palace, _root) = index_arbor();
        let structs: Vec<_> = palace
            .node_weights()
            .filter(|n| n.kind == NodeKind::Struct && n.visibility == Visibility::Public)
            .collect();

        assert!(
            structs.len() >= 5,
            "should find multiple public structs, got {}",
            structs.len()
        );
        let names: Vec<&str> = structs.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"Palace"), "should find Palace");
        assert!(names.contains(&"Node"), "should find Node");
    }

    #[test]
    fn symbols_kind_filter_works() {
        let (palace, _root) = index_arbor();
        let enums: Vec<_> = palace
            .node_weights()
            .filter(|n| n.kind == NodeKind::Enum)
            .collect();

        assert!(!enums.is_empty(), "should find enums");
        let names: Vec<&str> = enums.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"NodeKind"), "should find NodeKind enum");
        assert!(names.contains(&"EdgeKind"), "should find EdgeKind enum");
    }
}
