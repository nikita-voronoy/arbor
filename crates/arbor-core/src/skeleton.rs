use crate::graph::{NodeKind, Visibility};
use crate::palace::Palace;
use petgraph::Direction;
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::{Path, PathBuf};

const IGNORED_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".git",
    "__pycache__",
    "vendor",
    "dist",
    "build",
];

const TRANSPARENT_DIRS: &[&str] = &["src", "lib", "pkg", "crates", "packages", "internal", "cmd"];

const NOISE_NAMES: &[&str] = &[
    "new",
    "default",
    "into",
    "from",
    "clone",
    "to_string",
    "fmt",
    "drop",
    "eq",
    "hash",
    "cmp",
    "partial_cmp",
    "main",
    "anonymous",
];

const NOISE_CALLS: &[&str] = &["new", "default", "into", "from", "clone", "to_string"];

/// Per-module summary for boot screen generation
#[derive(Default)]
struct ModuleInfo<'a> {
    fn_count: usize,
    pub_fn_count: usize,
    test_count: usize,
    pub_structs: BTreeSet<&'a str>,
    traits: BTreeSet<&'a str>,
    enums: BTreeSet<&'a str>,
    domain_items: BTreeSet<&'a str>,
}

impl Palace {
    /// Generate a semantic boot screen: architecture summary, not just type lists.
    /// Target: ~150 tokens for small projects, ~300 for large.
    pub fn boot(&self, project_name: &str, project_type: &str) -> String {
        use std::collections::BTreeMap;

        let stats = self.stats();
        let mut out = String::new();

        // Line 1: identity + compressed stats
        let loc_display = if stats.total_lines >= 1000 {
            format!("~{}kLOC", stats.total_lines / 1000)
        } else {
            format!("~{}LOC", stats.total_lines)
        };
        writeln!(
            out,
            "{}|{}|{}f {}fn {}st {}tr {}en {}",
            project_name,
            project_type,
            stats.files,
            stats.functions,
            stats.structs,
            stats.traits,
            stats.enums,
            loc_display,
        )
        .ok();

        let common_prefix =
            Self::common_prefix(self.file_index.keys().map(std::path::PathBuf::as_path));

        let mut modules: BTreeMap<String, ModuleInfo> = BTreeMap::new();
        self.collect_boot_modules(&common_prefix, &mut modules);

        let node_degrees = self.find_hub_nodes();

        Self::render_boot_modules(&mut out, &modules);
        Self::render_boot_hubs(&mut out, &node_degrees);
        self.render_boot_edges(&mut out);

        out
    }

    fn collect_boot_modules<'a>(
        &'a self,
        common_prefix: &Path,
        modules: &mut std::collections::BTreeMap<String, ModuleInfo<'a>>,
    ) {
        for (file_path, indices) in &self.file_index {
            let rel = file_path.strip_prefix(common_prefix).unwrap_or(file_path);
            let rel_str = rel.to_string_lossy();

            if rel_str.contains("fixture") || rel_str.contains("/target/") {
                continue;
            }
            if let Some(first) = rel.components().next() {
                let first_str = first.as_os_str().to_string_lossy();
                if IGNORED_DIRS.iter().any(|d| first_str == *d) {
                    continue;
                }
            }

            let module_name = Self::derive_module_name(rel);
            let is_test =
                rel_str.contains("test") || rel_str.contains("spec") || rel_str.contains("bench");

            let info = modules.entry(module_name).or_default();
            for &idx in indices {
                if let Some(node) = self.get_node(idx) {
                    Self::classify_node_for_boot(node, is_test, info);
                }
            }
        }
    }

    fn derive_module_name(rel: &Path) -> String {
        let components: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();

        if components.len() <= 1 {
            components.first().map_or_else(
                || "root".to_string(),
                |s| s.rsplit('.').next_back().unwrap_or(s).to_string(),
            )
        } else {
            components
                .iter()
                .take(components.len().saturating_sub(1))
                .find(|c| !TRANSPARENT_DIRS.iter().any(|t| c == t))
                .cloned()
                .unwrap_or_else(|| {
                    components.last().map_or_else(
                        || "root".to_string(),
                        |s| s.rsplit('.').next_back().unwrap_or(s).to_string(),
                    )
                })
        }
    }

    fn classify_node_for_boot<'a>(
        node: &'a crate::graph::Node,
        is_test: bool,
        info: &mut ModuleInfo<'a>,
    ) {
        if is_test {
            if matches!(node.kind, NodeKind::Function) {
                info.test_count += 1;
            }
            return;
        }
        match node.kind {
            NodeKind::Function => {
                info.fn_count += 1;
                if node.visibility == Visibility::Public {
                    info.pub_fn_count += 1;
                }
            }
            NodeKind::Struct
                if node.visibility == Visibility::Public && node.name != "anonymous" =>
            {
                info.pub_structs.insert(node.name.as_str());
            }
            NodeKind::Trait if node.visibility == Visibility::Public => {
                info.traits.insert(node.name.as_str());
            }
            NodeKind::Enum if node.visibility == Visibility::Public => {
                info.enums.insert(node.name.as_str());
            }
            NodeKind::Role
            | NodeKind::Resource
            | NodeKind::Table
            | NodeKind::Message
            | NodeKind::Document => {
                info.domain_items.insert(node.name.as_str());
            }
            _ => {}
        }
    }

    fn find_hub_nodes(&self) -> Vec<(&str, &str, usize)> {
        use std::collections::BTreeMap;

        let mut best_by_name: BTreeMap<&str, (&str, usize)> = BTreeMap::new();
        for idx in self.graph.node_indices() {
            if let Some(node) = self.get_node(idx) {
                if matches!(
                    node.kind,
                    NodeKind::File | NodeKind::EnumVariant | NodeKind::Column | NodeKind::Impl
                ) || NOISE_NAMES.contains(&node.name.as_str())
                {
                    continue;
                }
                let meaningful_degree = self
                    .graph
                    .edges_directed(idx, Direction::Outgoing)
                    .filter(|e| !matches!(e.weight(), crate::graph::EdgeKind::Contains))
                    .count()
                    + self
                        .graph
                        .edges_directed(idx, Direction::Incoming)
                        .filter(|e| !matches!(e.weight(), crate::graph::EdgeKind::Contains))
                        .count();
                if meaningful_degree <= 1 {
                    continue;
                }
                let kind_tag = match node.kind {
                    NodeKind::Struct => "st",
                    NodeKind::Trait => "tr",
                    NodeKind::Function => "fn",
                    NodeKind::Enum => "en",
                    NodeKind::Role => "role",
                    NodeKind::Table => "tbl",
                    NodeKind::Message => "msg",
                    _ => "item",
                };
                let entry = best_by_name
                    .entry(node.name.as_str())
                    .or_insert((kind_tag, 0));
                if meaningful_degree > entry.1 {
                    *entry = (kind_tag, meaningful_degree);
                }
            }
        }
        let mut node_degrees: Vec<(&str, &str, usize)> = best_by_name
            .into_iter()
            .map(|(name, (kind, deg))| (name, kind, deg))
            .collect();
        node_degrees.sort_by(|a, b| b.2.cmp(&a.2));
        node_degrees.truncate(7);
        node_degrees
    }

    fn render_boot_modules(
        out: &mut String,
        modules: &std::collections::BTreeMap<String, ModuleInfo>,
    ) {
        let module_count = modules.len();
        for (name, info) in modules.iter().take(12) {
            let mut parts: Vec<String> = Vec::new();

            if !info.traits.is_empty() {
                let traits: Vec<&str> = info.traits.iter().copied().collect();
                parts.push(format!("trait:{}", traits.join(",")));
            }
            if !info.pub_structs.is_empty() {
                let structs: Vec<&str> = info.pub_structs.iter().copied().take(5).collect();
                let suffix = if info.pub_structs.len() > 5 {
                    format!("+{}", info.pub_structs.len() - 5)
                } else {
                    String::new()
                };
                parts.push(format!("{}{suffix}", structs.join(",")));
            }
            if !info.enums.is_empty() {
                let enums: Vec<&str> = info.enums.iter().copied().collect();
                parts.push(format!("en:{}", enums.join(",")));
            }
            if !info.domain_items.is_empty() {
                let items: Vec<&str> = info.domain_items.iter().copied().take(5).collect();
                parts.push(items.join(","));
            }
            if info.fn_count > 0 {
                if info.pub_fn_count > 0 && info.pub_fn_count < info.fn_count {
                    parts.push(format!("{}fn({}pub)", info.fn_count, info.pub_fn_count));
                } else {
                    parts.push(format!("{}fn", info.fn_count));
                }
            }
            if info.test_count > 0 {
                parts.push(format!("{}tests", info.test_count));
            }

            if parts.is_empty() {
                continue;
            }
            writeln!(out, "  {name}: {}", parts.join(" | ")).ok();
        }
        if module_count > 12 {
            writeln!(out, "  ...+{} more modules", module_count - 12).ok();
        }
    }

    fn render_boot_hubs(out: &mut String, node_degrees: &[(&str, &str, usize)]) {
        if !node_degrees.is_empty() {
            let hubs: Vec<String> = node_degrees
                .iter()
                .map(|(name, kind, deg)| format!("{kind}:{name}({deg})"))
                .collect();
            writeln!(out, "hubs: {}", hubs.join(" ")).ok();
        }
    }

    fn render_boot_edges(&self, out: &mut String) {
        let mut call_count = 0usize;
        let mut typeref_count = 0usize;
        for edge in self.graph.edge_references() {
            match edge.weight() {
                crate::graph::EdgeKind::Calls => call_count += 1,
                crate::graph::EdgeKind::TypeRef => typeref_count += 1,
                _ => {}
            }
        }
        if call_count > 0 || typeref_count > 0 {
            let mut edge_parts = Vec::new();
            if call_count > 0 {
                edge_parts.push(format!("{call_count} calls"));
            }
            if typeref_count > 0 {
                edge_parts.push(format!("{typeref_count} typerefs"));
            }
            writeln!(out, "edges: {}", edge_parts.join(", ")).ok();
        }
    }

    /// Generate a compact skeleton for a path or the whole project
    #[must_use]
    pub fn skeleton(&self, path: Option<&Path>, depth: usize) -> String {
        let mut out = String::new();

        let nodes: Vec<NodeIndex> = path.map_or_else(
            || self.graph.node_indices().collect(),
            |path| {
                self.file_index
                    .iter()
                    .filter(|(file_path, _)| file_path.starts_with(path))
                    .flat_map(|(_, indices)| indices.iter().copied())
                    .collect()
            },
        );

        // Group by file
        let mut by_file: std::collections::BTreeMap<&Path, Vec<NodeIndex>> =
            std::collections::BTreeMap::new();
        for idx in nodes {
            if let Some(node) = self.get_node(idx) {
                by_file.entry(&node.file).or_default().push(idx);
            }
        }

        let max_output = 50_000; // ~12k tokens hard limit
        for (file, indices) in &by_file {
            if out.len() > max_output {
                writeln!(out, "\n... truncated ({} files remaining)", by_file.len()).ok();
                break;
            }
            writeln!(out, "── {}", file.display()).ok();
            for &idx in indices {
                if out.len() > max_output {
                    break;
                }
                if let Some(node) = self.get_node(idx) {
                    self.write_node_skeleton(&mut out, idx, node, 1, depth);
                }
            }
        }

        out
    }

    fn write_node_skeleton(
        &self,
        out: &mut String,
        idx: NodeIndex,
        node: &crate::graph::Node,
        indent: usize,
        max_depth: usize,
    ) {
        if indent > max_depth {
            return;
        }

        let prefix = "  ".repeat(indent);
        let kind_tag = match node.kind {
            NodeKind::Function => "fn",
            NodeKind::Struct => "struct",
            NodeKind::Trait => "trait",
            NodeKind::Impl => "impl",
            NodeKind::Enum => "enum",
            NodeKind::Module => "mod",
            NodeKind::Constant => "const",
            NodeKind::EnumVariant => "variant",
            NodeKind::Macro => "macro",
            NodeKind::TypeAlias => "type",
            NodeKind::Role => "role",
            NodeKind::Task => "task",
            NodeKind::Handler => "handler",
            NodeKind::Variable => "var",
            NodeKind::Document => "doc",
            NodeKind::Section => "sec",
            _ => "item",
        };

        let vis = match node.visibility {
            Visibility::Public => "pub ",
            _ => "",
        };

        if let Some(sig) = &node.signature {
            writeln!(out, "{prefix}{vis}{kind_tag} {sig}").ok();
        } else {
            writeln!(out, "{}{}{} {}", prefix, vis, kind_tag, node.name).ok();
        }

        // Show call edges compactly (deduplicated)
        let mut call_set = std::collections::BTreeSet::new();
        for e in self.graph.edges_directed(idx, Direction::Outgoing) {
            if matches!(e.weight(), crate::graph::EdgeKind::Calls)
                && let Some(n) = self.get_node(e.target())
            {
                call_set.insert(n.name.as_str());
            }
        }
        if !call_set.is_empty() {
            let calls: Vec<&str> = call_set.into_iter().collect();
            writeln!(out, "{}  calls: [{}]", prefix, calls.join(", ")).ok();
        }
    }

    /// Generate a compact, token-optimized skeleton.
    /// Collapses signatures to one line, strips noise (self, full paths, trivial calls).
    #[must_use]
    pub fn compact_skeleton(
        &self,
        path: Option<&Path>,
        max_items: usize,
        skip_tests: bool,
    ) -> String {
        let mut out = String::new();

        let nodes: Vec<NodeIndex> = path.map_or_else(
            || self.graph.node_indices().collect(),
            |path| {
                self.file_index
                    .iter()
                    .filter(|(file_path, _)| file_path.starts_with(path))
                    .flat_map(|(_, indices)| indices.iter().copied())
                    .collect()
            },
        );

        // Group by file, skip File nodes themselves
        let mut by_file: std::collections::BTreeMap<&Path, Vec<NodeIndex>> =
            std::collections::BTreeMap::new();
        for idx in &nodes {
            if let Some(node) = self.get_node(*idx) {
                if matches!(node.kind, NodeKind::File) {
                    continue;
                }
                by_file.entry(&node.file).or_default().push(*idx);
            }
        }

        // Find common path prefix to strip from file paths
        let common_prefix = Self::common_prefix(by_file.keys().copied());

        let mut count = 0;
        let mut global_seen: std::collections::HashSet<(&str, NodeKind)> =
            std::collections::HashSet::new();

        for (file, indices) in &by_file {
            // Skip test/bench/fixture files
            if skip_tests {
                let file_str = file.to_string_lossy();
                if file_str.contains("/test")
                    || file_str.contains("/spec")
                    || file_str.contains("/fixture")
                    || file_str.contains("/bench")
                    || file_str.ends_with("_test.go")
                    || file_str.ends_with("_test.rs")
                    || file_str.ends_with(".test.ts")
                    || file_str.ends_with(".spec.ts")
                    || file_str.ends_with("_test.py")
                    || file_str.ends_with("test_.py")
                {
                    continue;
                }
            }

            // Collect non-dedup, non-impl items for this file
            let mut file_items = Vec::new();
            for &idx in indices {
                if let Some(node) = self.get_node(idx) {
                    // Skip impl blocks as standalone entries
                    if matches!(node.kind, NodeKind::Impl) {
                        continue;
                    }
                    // Skip test functions by name pattern
                    if skip_tests && matches!(node.kind, NodeKind::Function) {
                        let name = node.name.as_str();
                        if name.starts_with("test_")
                            || name.starts_with("Test")
                            || name.ends_with("_test")
                        {
                            continue;
                        }
                    }
                    let dedup_key = (node.name.as_str(), node.kind);
                    if !global_seen.insert(dedup_key) {
                        continue;
                    }
                    file_items.push((idx, node));
                }
            }

            // Skip files with no items
            if file_items.is_empty() {
                continue;
            }

            // Show relative path, disambiguating files with same name
            let display_path = file.strip_prefix(&common_prefix).unwrap_or(file);
            writeln!(out, "─{}", display_path.display()).ok();

            for (idx, node) in file_items {
                if count >= max_items {
                    writeln!(out, "  ...+{} more", nodes.len() - count).ok();
                    return out;
                }

                // Skip standalone variants/columns — they get inlined into parent
                if matches!(node.kind, NodeKind::EnumVariant | NodeKind::Column) {
                    continue;
                }

                if self.write_compact_node(&mut out, idx, node) {
                    count += 1;
                }
            }
        }

        out
    }

    /// Write a single node in compact format. Returns true if written.
    fn write_compact_node(
        &self,
        out: &mut String,
        idx: NodeIndex,
        node: &crate::graph::Node,
    ) -> bool {
        let tag = match node.kind {
            NodeKind::Function => "fn",
            NodeKind::Struct => "st",
            NodeKind::Trait => "tr",
            NodeKind::Enum => "en",
            NodeKind::Module => "mod",
            NodeKind::Constant => "co",
            NodeKind::Macro => "def",
            NodeKind::TypeAlias => "ty",
            NodeKind::Role => "role",
            NodeKind::Task => "task",
            NodeKind::Handler => "hnd",
            NodeKind::Variable => "var",
            NodeKind::Template => "tpl",
            NodeKind::Resource => "res",
            NodeKind::Document => "doc",
            NodeKind::Section => "sec",
            NodeKind::CodeBlock => "code",
            NodeKind::Table => "tbl",
            NodeKind::Endpoint => "ep",
            NodeKind::Message => "msg",
            NodeKind::EnumVariant | NodeKind::Column | NodeKind::Impl | NodeKind::File => {
                return false;
            }
        };

        let vis = if node.visibility == Visibility::Public {
            "+"
        } else {
            ""
        };

        if matches!(node.kind, NodeKind::Enum | NodeKind::Table) {
            let children: Vec<&str> = self
                .graph
                .edges_directed(idx, Direction::Outgoing)
                .filter(|e| matches!(e.weight(), crate::graph::EdgeKind::Contains))
                .filter_map(|e| self.get_node(e.target()))
                .filter(|n| matches!(n.kind, NodeKind::EnumVariant | NodeKind::Column))
                .map(|n| n.name.as_str())
                .collect();

            if children.is_empty() {
                writeln!(out, " {vis}{tag}:{}", node.name).ok();
            } else {
                writeln!(out, " {vis}{tag}:{}[{}]", node.name, children.join(",")).ok();
            }
            return true;
        }

        let label = node
            .signature
            .as_ref()
            .map_or_else(|| node.name.clone(), |sig| Self::compress_signature(sig));

        let mut call_set = BTreeSet::new();
        for e in self.graph.edges_directed(idx, Direction::Outgoing) {
            if matches!(e.weight(), crate::graph::EdgeKind::Calls)
                && let Some(n) = self.get_node(e.target())
                && !NOISE_CALLS.contains(&n.name.as_str())
            {
                call_set.insert(n.name.as_str());
            }
        }
        let calls: Vec<&str> = call_set.into_iter().collect();

        if calls.is_empty() {
            writeln!(out, " {vis}{tag}:{label}").ok();
        } else {
            writeln!(out, " {vis}{tag}:{label}→[{}]", calls.join(",")).ok();
        }

        true
    }

    /// Compress a signature to a single line and strip noise.
    fn compress_signature(sig: &str) -> String {
        // 1. Collapse to single line
        let oneline: String = sig.lines().map(str::trim).collect::<Vec<_>>().join(" ");

        // 2. Strip common keywords
        let compressed = oneline
            .replace("pub fn ", "")
            .replace("pub async fn ", "")
            .replace("async fn ", "")
            .replace("fn ", "")
            .replace("pub ", "");

        // 3. Shorten fully-qualified paths: keep only the last segment
        //    e.g. "petgraph::stable_graph::NodeIndex" → "NodeIndex"
        //    e.g. "tree_sitter::TreeCursor" → "TreeCursor"
        //    But preserve &, *, mut, etc.
        let shortened = Self::shorten_paths(&compressed);

        // 4. Strip &self / &mut self (implied for methods)
        let shortened = shortened
            .replace("(&self, ", "(")
            .replace("(&self)", "()")
            .replace("(&mut self, ", "(&mut, ")
            .replace("(&mut self)", "(&mut)")
            .replace("(mut self, ", "(mut, ")
            .replace("(mut self)", "(mut)");

        // 5. Truncate if still too long (char-boundary safe)
        if shortened.len() > 120 {
            let mut end = 117;
            while !shortened.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &shortened[..end])
        } else {
            shortened
        }
    }

    /// Shorten "`foo::bar::Baz`" to "Baz" in type positions
    fn shorten_paths(s: &str) -> String {
        // Match sequences like "some::path::Type" and keep only "Type"
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch.is_alphanumeric() || ch == '_' {
                let mut segment = String::new();
                segment.push(ch);
                while let Some(&next) = chars.peek() {
                    if next.is_alphanumeric() || next == '_' {
                        segment.push(next);
                        chars.next();
                    } else if next == ':' {
                        chars.next(); // consume first ':'
                        if chars.peek() == Some(&':') {
                            chars.next(); // consume second ':'
                        // Start a new segment — discard old
                        } else {
                            // Single ':' — keep what we have and the ':'
                            result.push_str(&segment);
                            result.push(':');
                        }
                        segment.clear();
                    } else {
                        break;
                    }
                }
                result.push_str(&segment);
            } else {
                result.push(ch);
            }
        }

        result
    }

    /// Find common path prefix across all file paths
    fn common_prefix<'a>(paths: impl Iterator<Item = &'a Path>) -> PathBuf {
        let mut prefix: Option<PathBuf> = None;
        for path in paths {
            match &prefix {
                None => prefix = Some(path.parent().unwrap_or(path).to_path_buf()),
                Some(p) => {
                    let mut common = PathBuf::new();
                    for (a, b) in p.components().zip(path.components()) {
                        if a == b {
                            common.push(a);
                        } else {
                            break;
                        }
                    }
                    prefix = Some(common);
                }
            }
        }
        prefix.unwrap_or_default()
    }
}
