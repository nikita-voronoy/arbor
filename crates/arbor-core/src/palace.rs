use crate::graph::{CodeGraph, EdgeKind, Node, NodeKind};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

/// A Wing represents a project or repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wing {
    pub name: String,
    pub root: PathBuf,
    pub rooms: Vec<RoomId>,
}

/// A Room represents a module, package, or role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomId(pub usize);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub name: String,
    pub path: PathBuf,
    pub nodes: Vec<NodeIndex>,
}

/// A Tunnel connects related symbols across different wings (projects)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tunnel {
    pub from_wing: usize,
    pub to_wing: usize,
    pub from_node: NodeIndex,
    pub to_node: NodeIndex,
    pub reason: String,
}

/// The Palace is the top-level container holding the graph and organizational structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Palace {
    pub(crate) graph: CodeGraph,
    pub wings: Vec<Wing>,
    pub rooms: Vec<Room>,
    pub tunnels: Vec<Tunnel>,
    /// Map from file path to the nodes defined in that file
    pub(crate) file_index: FxHashMap<PathBuf, Vec<NodeIndex>>,
    /// Map from symbol name to node indices (for fast lookup)
    pub(crate) name_index: FxHashMap<String, Vec<NodeIndex>>,
    /// Pending call edges to resolve after all files are indexed (`caller_idx`, `callee_name`)
    #[serde(default)]
    pub(crate) pending_calls: Vec<(NodeIndex, String)>,
}

impl Palace {
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: CodeGraph::default(),
            wings: Vec::new(),
            rooms: Vec::new(),
            tunnels: Vec::new(),
            file_index: FxHashMap::default(),
            name_index: FxHashMap::default(),
            pending_calls: Vec::new(),
        }
    }

    /// Add a node to the graph and update indices
    pub fn add_node(&mut self, node: Node) -> NodeIndex {
        let file = node.file.clone();
        let name = node.name.clone();
        let idx = self.graph.add_node(node);

        self.file_index.entry(file).or_default().push(idx);
        self.name_index.entry(name).or_default().push(idx);

        idx
    }

    /// Add an edge between two nodes
    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, kind: EdgeKind) {
        self.graph.add_edge(from, to, kind);
    }

    /// Get all nodes in a file
    pub fn nodes_in_file(&self, path: &Path) -> &[NodeIndex] {
        self.file_index
            .get(path)
            .map_or(&[], std::vec::Vec::as_slice)
    }

    /// Find nodes by name
    pub fn find_by_name(&self, name: &str) -> &[NodeIndex] {
        self.name_index
            .get(name)
            .map_or(&[], std::vec::Vec::as_slice)
    }

    /// Get a node by index
    #[must_use]
    pub fn get_node(&self, idx: NodeIndex) -> Option<&Node> {
        self.graph.node_weight(idx)
    }

    /// Get the number of nodes in the graph
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Iterate over all nodes in the graph
    pub fn node_weights(&self) -> impl Iterator<Item = &Node> {
        self.graph.node_weights()
    }

    /// Check if a node is a real symbol (not a File-level node).
    #[must_use]
    pub fn is_real_symbol(&self, idx: NodeIndex) -> bool {
        self.get_node(idx)
            .is_some_and(|n| !matches!(n.kind, NodeKind::File))
    }

    /// Get the names of symbols called by a given node.
    #[must_use]
    pub fn callees(&self, idx: NodeIndex) -> Vec<&str> {
        use petgraph::Direction;
        use petgraph::visit::EdgeRef;

        self.graph
            .edges_directed(idx, Direction::Outgoing)
            .filter(|e| matches!(e.weight(), EdgeKind::Calls))
            .filter_map(|e| self.get_node(e.target()))
            .map(|n| n.name.as_str())
            .collect()
    }

    /// Iterate over all indexed file paths
    pub fn file_paths(&self) -> impl Iterator<Item = &Path> {
        self.file_index.keys().map(PathBuf::as_path)
    }

    /// Record a call edge to be resolved after all files are indexed
    pub fn add_pending_call(&mut self, caller: NodeIndex, callee_name: String) {
        self.pending_calls.push((caller, callee_name));
    }

    /// Resolve all pending call edges. Call this after all files have been analyzed.
    pub fn resolve_pending_calls(&mut self) {
        use rustc_hash::FxHashSet;

        let pending = std::mem::take(&mut self.pending_calls);
        if pending.is_empty() {
            return;
        }

        // Dedup only within the pending batch — existing edges from extract_calls
        // are already deduplicated per function, so we only guard against cross-file dupes.
        let mut added: FxHashSet<(NodeIndex, NodeIndex)> = FxHashSet::default();

        for (caller_idx, callee_name) in pending {
            let targets: Vec<NodeIndex> = self.find_by_name(&callee_name).to_vec();
            for target in targets {
                if target != caller_idx && added.insert((caller_idx, target)) {
                    // Check the graph too — extract_calls may have already added this edge
                    let exists = self
                        .graph
                        .edges_connecting(caller_idx, target)
                        .any(|e| matches!(e.weight(), EdgeKind::Calls));
                    if !exists {
                        self.graph.add_edge(caller_idx, target, EdgeKind::Calls);
                    }
                }
            }
        }
    }

    /// Remove all nodes associated with a file (for incremental re-analysis)
    pub fn remove_file(&mut self, path: &Path) {
        if let Some(indices) = self.file_index.remove(path) {
            // Collect (idx, name) pairs first to avoid borrow conflict
            let to_remove: Vec<(NodeIndex, String)> = indices
                .iter()
                .filter_map(|&idx| {
                    self.graph
                        .node_weight(idx)
                        .map(|node| (idx, node.name.clone()))
                })
                .collect();

            for (idx, name) in &to_remove {
                if let Some(name_entries) = self.name_index.get_mut(name.as_str()) {
                    name_entries.retain(|i| i != idx);
                    if name_entries.is_empty() {
                        self.name_index.remove(name.as_str());
                    }
                }
                self.graph.remove_node(*idx);
            }
        }
    }

    /// Get total counts for boot screen
    #[must_use]
    pub fn stats(&self) -> PalaceStats {
        let mut stats = PalaceStats::default();
        for node in self.graph.node_weights() {
            match node.kind {
                NodeKind::File => stats.files += 1,
                NodeKind::Function => stats.functions += 1,
                NodeKind::Struct => stats.structs += 1,
                NodeKind::Trait => stats.traits += 1,
                NodeKind::Enum => stats.enums += 1,
                NodeKind::Module => stats.modules += 1,
                NodeKind::Impl
                | NodeKind::EnumVariant
                | NodeKind::Constant
                | NodeKind::TypeAlias
                | NodeKind::Macro
                | NodeKind::Role
                | NodeKind::Task
                | NodeKind::Handler
                | NodeKind::Variable
                | NodeKind::Template
                | NodeKind::Resource
                | NodeKind::Document
                | NodeKind::Section
                | NodeKind::CodeBlock
                | NodeKind::Table
                | NodeKind::Column
                | NodeKind::Endpoint
                | NodeKind::Message => stats.other += 1,
            }
            stats.total_lines += node.span.lines() as usize;
        }
        stats
    }

    /// Add a wing (project) to the palace
    pub fn add_wing(&mut self, name: impl Into<String>, root: impl Into<PathBuf>) -> usize {
        let idx = self.wings.len();
        self.wings.push(Wing {
            name: name.into(),
            root: root.into(),
            rooms: Vec::new(),
        });
        idx
    }

    /// Merge another palace into this one as a new wing.
    /// Returns the wing index.
    pub fn merge_wing(
        &mut self,
        name: impl Into<String>,
        root: impl Into<PathBuf>,
        other: &Self,
    ) -> usize {
        let wing_idx = self.add_wing(name, root);

        // Map old indices to new indices
        let mut index_map: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        // Copy nodes
        for old_idx in other.graph.node_indices() {
            if let Some(node) = other.graph.node_weight(old_idx) {
                let new_idx = self.add_node(node.clone());
                index_map.insert(old_idx, new_idx);
            }
        }

        // Copy edges
        for edge in other.graph.edge_references() {
            if let (Some(&new_from), Some(&new_to)) =
                (index_map.get(&edge.source()), index_map.get(&edge.target()))
            {
                self.add_edge(new_from, new_to, *edge.weight());
            }
        }

        wing_idx
    }

    /// Auto-discover tunnels between wings by matching symbol names.
    /// Safe to call multiple times — clears previous tunnels first.
    pub fn discover_tunnels(&mut self) {
        type WingEntry = (NodeIndex, NodeKind, usize);

        const fn is_tunnel_pair(a: NodeKind, b: NodeKind) -> bool {
            matches!(
                (a, b),
                (
                    NodeKind::Struct | NodeKind::Message | NodeKind::Table,
                    NodeKind::Struct
                ) | (NodeKind::Trait, NodeKind::Trait)
                    | (NodeKind::Enum, NodeKind::Enum)
                    | (NodeKind::Message | NodeKind::Struct, NodeKind::Message)
                    | (NodeKind::Table | NodeKind::Struct, NodeKind::Table)
            )
        }

        self.tunnels.clear();

        if self.wings.len() < 2 {
            return;
        }

        // Phase 1: collect node info per wing, indexed by name for O(N) matching
        let wing_nodes: Vec<FxHashMap<&str, Vec<WingEntry>>> = self
            .wings
            .iter()
            .enumerate()
            .map(|(wing_idx, wing)| {
                let mut by_name: FxHashMap<&str, Vec<WingEntry>> = FxHashMap::default();
                for (path, indices) in &self.file_index {
                    if !path.starts_with(&wing.root) {
                        continue;
                    }
                    for &idx in indices {
                        if let Some(n) = self.graph.node_weight(idx)
                            && !n.name.is_empty()
                        {
                            by_name
                                .entry(n.name.as_str())
                                .or_default()
                                .push((idx, n.kind, wing_idx));
                        }
                    }
                }
                by_name
            })
            .collect();

        // Phase 2: intersect by name across wing pairs
        let mut new_tunnels: Vec<Tunnel> = Vec::new();
        let mut new_edges: Vec<(NodeIndex, NodeIndex)> = Vec::new();

        for i in 0..wing_nodes.len() {
            for j in (i + 1)..wing_nodes.len() {
                // Iterate the smaller map, probe the larger
                let (smaller, larger) = if wing_nodes[i].len() <= wing_nodes[j].len() {
                    (&wing_nodes[i], &wing_nodes[j])
                } else {
                    (&wing_nodes[j], &wing_nodes[i])
                };

                for (name, nodes_a) in smaller {
                    if let Some(nodes_b) = larger.get(name) {
                        for &(idx_a, kind_a, wing_a) in nodes_a {
                            for &(idx_b, kind_b, wing_b) in nodes_b {
                                if is_tunnel_pair(kind_a, kind_b) {
                                    let (from_wing, to_wing, from_node, to_node) =
                                        if wing_a < wing_b {
                                            (wing_a, wing_b, idx_a, idx_b)
                                        } else {
                                            (wing_b, wing_a, idx_b, idx_a)
                                        };
                                    new_tunnels.push(Tunnel {
                                        from_wing,
                                        to_wing,
                                        from_node,
                                        to_node,
                                        reason: format!("shared type: {name}"),
                                    });
                                    new_edges.push((from_node, to_node));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Phase 3: apply mutations
        for (from, to) in new_edges {
            self.graph.add_edge(from, to, EdgeKind::References);
        }
        self.tunnels.extend(new_tunnels);
    }

    /// Format tunnels for display
    #[must_use]
    pub fn format_tunnels(&self) -> String {
        if self.tunnels.is_empty() {
            return "No cross-project tunnels found.".to_string();
        }

        let mut out = format!("Cross-project tunnels ({}):\n", self.tunnels.len());
        for tunnel in &self.tunnels {
            let from_wing = self
                .wings
                .get(tunnel.from_wing)
                .map_or("?", |w| w.name.as_str());
            let to_wing = self
                .wings
                .get(tunnel.to_wing)
                .map_or("?", |w| w.name.as_str());
            let from_name = self
                .get_node(tunnel.from_node)
                .map_or("?", |n| n.name.as_str());
            let _ = writeln!(
                out,
                "  {from_wing} ←→ {to_wing} [{from_name}] ({})",
                tunnel.reason
            );
        }
        out
    }

    /// Create a room from a directory path, collecting all nodes under it
    pub fn create_room(&mut self, name: impl Into<String>, path: impl Into<PathBuf>) -> RoomId {
        let path = path.into();
        let name = name.into();
        let nodes: Vec<NodeIndex> = self
            .file_index
            .iter()
            .filter(|(file_path, _)| file_path.starts_with(&path))
            .flat_map(|(_, indices)| indices.iter().copied())
            .collect();

        let id = RoomId(self.rooms.len());
        self.rooms.push(Room { name, path, nodes });
        id
    }
}

impl Default for Palace {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub struct PalaceStats {
    pub files: usize,
    pub functions: usize,
    pub structs: usize,
    pub traits: usize,
    pub enums: usize,
    pub modules: usize,
    pub other: usize,
    pub total_lines: usize,
}
