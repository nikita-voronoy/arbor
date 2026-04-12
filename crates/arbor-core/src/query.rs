use crate::graph::{EdgeKind, NodeKind};
use crate::palace::Palace;
use petgraph::Direction;
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::path::Path;

/// Result of a references query
#[derive(Debug)]
pub struct Reference {
    pub node: NodeIndex,
    pub kind: ReferenceKind,
}

#[derive(Debug)]
pub enum ReferenceKind {
    Definition,
    Call,
    Import,
    TypeReference,
    Implementation,
    Other,
}

impl Palace {
    /// Find all references to a symbol by name.
    /// Only counts the first occurrence per file as Definition, the rest as Other.
    pub fn references(&self, symbol: &str) -> Vec<Reference> {
        let mut results = Vec::new();
        let mut seen_def_files: FxHashSet<&Path> = FxHashSet::default();

        // Find definitions — deduplicate by file
        for &idx in self.find_by_name(symbol) {
            if let Some(node) = self.get_node(idx) {
                let is_real_def = matches!(
                    node.kind,
                    NodeKind::Function
                        | NodeKind::Struct
                        | NodeKind::Trait
                        | NodeKind::Enum
                        | NodeKind::EnumVariant
                        | NodeKind::Module
                        | NodeKind::Table
                        | NodeKind::Message
                        | NodeKind::Role
                        | NodeKind::Macro
                );
                if is_real_def && seen_def_files.insert(&node.file) {
                    results.push(Reference {
                        node: idx,
                        kind: ReferenceKind::Definition,
                    });
                }
            }
        }

        // Find incoming edges to these nodes
        let def_indices: FxHashSet<NodeIndex> = self.find_by_name(symbol).iter().copied().collect();

        let mut seen_refs = FxHashSet::default();
        for &def_idx in &def_indices {
            for edge in self.graph.edges_directed(def_idx, Direction::Incoming) {
                let source = edge.source();
                if seen_refs.insert(source) {
                    let kind = match edge.weight() {
                        EdgeKind::Calls => ReferenceKind::Call,
                        EdgeKind::Imports => ReferenceKind::Import,
                        EdgeKind::TypeRef => ReferenceKind::TypeReference,
                        EdgeKind::Implements => ReferenceKind::Implementation,
                        _ => ReferenceKind::Other,
                    };
                    results.push(Reference { node: source, kind });
                }
            }
        }

        results
    }

    /// Get transitive dependencies of a node (outgoing)
    pub fn dependencies(&self, start: NodeIndex, max_depth: usize) -> Vec<(NodeIndex, usize)> {
        self.traverse(start, Direction::Outgoing, max_depth)
    }

    /// Get transitive dependents of a node (incoming) — "what breaks if I change this"
    pub fn impact(&self, start: NodeIndex, max_depth: usize) -> Vec<(NodeIndex, usize)> {
        self.traverse(start, Direction::Incoming, max_depth)
    }

    fn traverse(
        &self,
        start: NodeIndex,
        direction: Direction,
        max_depth: usize,
    ) -> Vec<(NodeIndex, usize)> {
        let mut visited = FxHashSet::default();
        let mut queue = VecDeque::new();
        let mut results = Vec::new();

        visited.insert(start);
        queue.push_back((start, 0usize));

        while let Some((current, depth)) = queue.pop_front() {
            if depth > 0 {
                results.push((current, depth));
            }
            if depth >= max_depth {
                continue;
            }

            for neighbor in self.graph.neighbors_directed(current, direction) {
                if visited.insert(neighbor) {
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }

        results
    }

    /// Fuzzy search symbols by name substring.
    /// Deduplicates by (name, kind) — returns one result per unique symbol, not per occurrence.
    pub fn search(&self, query: &str) -> Vec<NodeIndex> {
        let query_lower = query.to_lowercase();

        // Collect unique (name, kind) → best NodeIndex
        let mut seen: FxHashMap<(&str, NodeKind), (NodeIndex, usize)> = FxHashMap::default();

        for (name, indices) in &self.name_index {
            let name_lower = name.to_lowercase();
            if !name_lower.contains(&query_lower) {
                continue;
            }

            let score = if name == query {
                0
            } else if name_lower.starts_with(&query_lower) {
                1
            } else {
                2
            };

            for &idx in indices {
                if let Some(node) = self.graph.node_weight(idx) {
                    // Skip File nodes
                    if matches!(node.kind, NodeKind::File) {
                        continue;
                    }

                    let key = (name.as_str(), node.kind);

                    let entry = seen.entry(key).or_insert((idx, score));
                    // Keep the one with best score (or first seen)
                    if score < entry.1 {
                        *entry = (idx, score);
                    }
                }
            }
        }

        let mut results: Vec<(NodeIndex, usize)> = seen.into_values().collect();
        results.sort_by_key(|(_, score)| *score);
        results.into_iter().map(|(idx, _)| idx).collect()
    }

    /// Find the "primary" definition node for a symbol — the first real definition found
    pub fn find_primary(&self, symbol: &str) -> Option<NodeIndex> {
        self.find_by_name(symbol).iter().copied().find(|&idx| {
            self.get_node(idx)
                .map(|n| {
                    matches!(
                        n.kind,
                        NodeKind::Function
                            | NodeKind::Struct
                            | NodeKind::Trait
                            | NodeKind::Enum
                            | NodeKind::EnumVariant
                            | NodeKind::Table
                            | NodeKind::Message
                            | NodeKind::Macro
                    )
                })
                .unwrap_or(false)
        })
    }
}
