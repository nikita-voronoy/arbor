use arbor_analyzers::AnalyzerRegistry;
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
use std::path::PathBuf;

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
        let is_fresh = cached_palace.is_none();
        let mut palace = cached_palace.unwrap_or_default();

        let changed = if is_fresh {
            // Fresh index — just analyze everything, no hashing overhead
            registry.analyze_project(&root, &mut palace)?;
            palace.resolve_pending_calls();

            // Hash indexed files for next time (only files Palace knows about)
            let mut hashes = arbor_persist::hasher::FileHashes::new();
            for path in palace.file_index.keys() {
                let _ = hashes.check_file(path);
            }
            hashes.save(&root)?;
            0usize
        } else {
            // Incremental update — only re-analyze changed files
            let mut hashes = arbor_persist::hasher::FileHashes::load(&root).unwrap_or_default();

            // Only walk files the analyzers care about (not ALL files)
            let current_files: std::collections::HashSet<PathBuf> =
                palace.file_index.keys().cloned().collect();

            // Check for deleted files
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

            // Check for modified files
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

            // Also check for new files (not in cache) by re-walking
            // This is needed to pick up newly created files
            let all_files = arbor_persist::watcher::walk_files(&root);
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
                        for analyzer in registry.for_facets(&facets) {
                            if let Err(e) = analyzer.analyze_file(path, &source, &mut palace) {
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
            hashes.save(&root)?;
            count
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

    fn project_name(&self) -> &str {
        self.root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
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

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DependenciesParams {
    /// Symbol name
    pub symbol: String,
    /// Direction: 'outgoing' (default) or 'incoming'
    pub direction: Option<String>,
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
            .filter(|r| {
                palace
                    .get_node(r.node)
                    .is_some_and(|n| !matches!(n.kind, arbor_core::graph::NodeKind::File))
            })
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
                    "  {:?} in {} ({}:{})",
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
        let incoming = params.0.direction.as_deref() == Some("incoming");

        let deps = if incoming {
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
        let dir_label = if incoming {
            "Dependents of"
        } else {
            "Dependencies of"
        };
        let deps: Vec<_> = deps
            .into_iter()
            .filter(|(idx, _)| {
                palace
                    .get_node(*idx)
                    .is_some_and(|n| !matches!(n.kind, arbor_core::graph::NodeKind::File))
            })
            .collect();

        let mut out = format!("{dir_label} '{}' ({} found):\n", node.name, deps.len());
        for (dep_idx, depth) in &deps {
            if let Some(dep) = palace.get_node(*dep_idx) {
                let kind = match dep.kind {
                    arbor_core::graph::NodeKind::Function => "fn",
                    arbor_core::graph::NodeKind::Struct => "struct",
                    arbor_core::graph::NodeKind::Trait => "trait",
                    arbor_core::graph::NodeKind::Macro => "macro",
                    arbor_core::graph::NodeKind::EnumVariant => "variant",
                    _ => "item",
                };
                let _ = writeln!(
                    out,
                    "  [depth {depth}] {kind} {} ({}:{})",
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
            .filter(|(idx, _)| {
                palace
                    .get_node(*idx)
                    .is_some_and(|n| !matches!(n.kind, arbor_core::graph::NodeKind::File))
            })
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
                let kind = match node.kind {
                    arbor_core::graph::NodeKind::Function => "fn",
                    arbor_core::graph::NodeKind::Struct => "struct",
                    arbor_core::graph::NodeKind::Trait => "trait",
                    arbor_core::graph::NodeKind::Enum => "enum",
                    arbor_core::graph::NodeKind::EnumVariant => "variant",
                    arbor_core::graph::NodeKind::Macro => "macro",
                    arbor_core::graph::NodeKind::Module => "mod",
                    _ => "item",
                };
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
                let _ = arbor_persist::store::save(&palace, &self.root);
                let stats = palace.stats();
                drop(palace);
                // Rebuild file hashes (no lock needed)
                let mut hashes = arbor_persist::hasher::FileHashes::new();
                for path in arbor_persist::watcher::walk_files(&self.root) {
                    let _ = hashes.check_file(&path);
                }
                let _ = hashes.save(&self.root);
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
