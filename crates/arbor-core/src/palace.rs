use crate::graph::{CodeGraph, EdgeKind, Node, NodeKind};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub graph: CodeGraph,
    pub wings: Vec<Wing>,
    pub rooms: Vec<Room>,
    pub tunnels: Vec<Tunnel>,
    /// Map from file path to the nodes defined in that file
    pub file_index: HashMap<PathBuf, Vec<NodeIndex>>,
    /// Map from symbol name to node indices (for fast lookup)
    pub name_index: HashMap<String, Vec<NodeIndex>>,
    /// Pending call edges to resolve after all files are indexed (caller_idx, callee_name)
    #[serde(default)]
    pub pending_calls: Vec<(NodeIndex, String)>,
}

impl Palace {
    pub fn new() -> Self {
        Self {
            graph: CodeGraph::default(),
            wings: Vec::new(),
            rooms: Vec::new(),
            tunnels: Vec::new(),
            file_index: HashMap::new(),
            name_index: HashMap::new(),
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
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Find nodes by name
    pub fn find_by_name(&self, name: &str) -> &[NodeIndex] {
        self.name_index
            .get(name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get a node by index
    pub fn get_node(&self, idx: NodeIndex) -> Option<&Node> {
        self.graph.node_weight(idx)
    }

    /// Record a call edge to be resolved after all files are indexed
    pub fn add_pending_call(&mut self, caller: NodeIndex, callee_name: String) {
        self.pending_calls.push((caller, callee_name));
    }

    /// Resolve all pending call edges. Call this after all files have been analyzed.
    pub fn resolve_pending_calls(&mut self) {
        let pending = std::mem::take(&mut self.pending_calls);
        for (caller_idx, callee_name) in pending {
            let targets: Vec<NodeIndex> = self.find_by_name(&callee_name).to_vec();
            for target in targets {
                if target != caller_idx {
                    // Avoid duplicate edges
                    let already_exists = self
                        .graph
                        .edges_directed(caller_idx, petgraph::Direction::Outgoing)
                        .any(|e| e.target() == target && matches!(e.weight(), EdgeKind::Calls));
                    if !already_exists {
                        self.graph.add_edge(caller_idx, target, EdgeKind::Calls);
                    }
                }
            }
        }
    }

    /// Remove all nodes associated with a file (for incremental re-analysis)
    pub fn remove_file(&mut self, path: &Path) {
        if let Some(indices) = self.file_index.remove(path) {
            for idx in &indices {
                if let Some(node) = self.graph.node_weight(*idx) {
                    let name = node.name.clone();
                    if let Some(name_entries) = self.name_index.get_mut(&name) {
                        name_entries.retain(|i| i != idx);
                        if name_entries.is_empty() {
                            self.name_index.remove(&name);
                        }
                    }
                }
                self.graph.remove_node(*idx);
            }
        }
    }

    /// Get total counts for boot screen
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
                _ => stats.other += 1,
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
        other: &Palace,
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
                self.add_edge(new_from, new_to, edge.weight().clone());
            }
        }

        wing_idx
    }

    /// Auto-discover tunnels between wings by matching symbol names
    pub fn discover_tunnels(&mut self) {
        if self.wings.len() < 2 {
            return;
        }

        // Phase 1: collect node info (immutable borrow)
        let wing_nodes: Vec<Vec<(NodeIndex, String, NodeKind)>> = self
            .wings
            .iter()
            .map(|wing| {
                self.file_index
                    .iter()
                    .filter(|(path, _)| path.starts_with(&wing.root))
                    .flat_map(|(_, indices)| {
                        indices.iter().filter_map(|&idx| {
                            self.graph
                                .node_weight(idx)
                                .map(|n| (idx, n.name.clone(), n.kind))
                        })
                    })
                    .collect()
            })
            .collect();

        // Phase 2: find matches (no borrows)
        let mut new_tunnels: Vec<Tunnel> = Vec::new();
        let mut new_edges: Vec<(NodeIndex, NodeIndex)> = Vec::new();

        for i in 0..wing_nodes.len() {
            for j in (i + 1)..wing_nodes.len() {
                for (idx_a, name_a, kind_a) in &wing_nodes[i] {
                    for (idx_b, name_b, kind_b) in &wing_nodes[j] {
                        if name_a == name_b && !name_a.is_empty() {
                            let meaningful = matches!(
                                (kind_a, kind_b),
                                (NodeKind::Struct, NodeKind::Struct)
                                    | (NodeKind::Trait, NodeKind::Trait)
                                    | (NodeKind::Enum, NodeKind::Enum)
                                    | (NodeKind::Message, NodeKind::Message)
                                    | (NodeKind::Table, NodeKind::Table)
                                    | (NodeKind::Struct, NodeKind::Message)
                                    | (NodeKind::Message, NodeKind::Struct)
                                    | (NodeKind::Struct, NodeKind::Table)
                                    | (NodeKind::Table, NodeKind::Struct)
                            );
                            if meaningful {
                                new_tunnels.push(Tunnel {
                                    from_wing: i,
                                    to_wing: j,
                                    from_node: *idx_a,
                                    to_node: *idx_b,
                                    reason: format!("shared type: {}", name_a),
                                });
                                new_edges.push((*idx_a, *idx_b));
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
    pub fn format_tunnels(&self) -> String {
        if self.tunnels.is_empty() {
            return "No cross-project tunnels found.".to_string();
        }

        let mut out = format!("Cross-project tunnels ({}):\n", self.tunnels.len());
        for tunnel in &self.tunnels {
            let from_wing = self
                .wings
                .get(tunnel.from_wing)
                .map(|w| w.name.as_str())
                .unwrap_or("?");
            let to_wing = self
                .wings
                .get(tunnel.to_wing)
                .map(|w| w.name.as_str())
                .unwrap_or("?");
            let from_name = self
                .get_node(tunnel.from_node)
                .map(|n| n.name.as_str())
                .unwrap_or("?");
            out.push_str(&format!(
                "  {} ←→ {} [{}] ({})\n",
                from_wing, to_wing, from_name, tunnel.reason
            ));
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
