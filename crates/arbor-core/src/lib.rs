pub mod graph;
pub mod palace;
pub mod query;
pub mod skeleton;

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
