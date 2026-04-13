pub mod graph;
pub mod palace;
pub mod query;
pub mod skeleton;

/// Re-export `NodeIndex` so downstream crates don't need petgraph directly.
pub use petgraph::stable_graph::NodeIndex;

/// Structured error type for arbor-core operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("node not found: {symbol}")]
    NodeNotFound { symbol: String },

    #[error("serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Convenience alias for arbor-core results
pub type Result<T> = std::result::Result<T, Error>;
