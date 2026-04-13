use petgraph::stable_graph::StableGraph;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unified node kinds across all project types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    // Code
    File,
    Module,
    Function,
    Struct,
    Trait,
    Impl,
    Enum,
    EnumVariant,
    Constant,
    TypeAlias,
    Macro,

    // IaC
    Role,
    Task,
    Handler,
    Variable,
    Template,
    Resource,

    // Docs
    Document,
    Section,
    CodeBlock,

    // Schema
    Table,
    Column,
    Endpoint,
    Message,
}

/// A node in the code graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    pub name: String,
    pub file: PathBuf,
    pub span: Span,
    pub signature: Option<String>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Span {
    pub start_line: u32,
    pub end_line: u32,
    pub start_col: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Crate,
}

/// Edge kinds representing relationships between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeKind {
    Contains,   // parent contains child (module→function)
    Calls,      // function calls function
    Imports,    // file/module imports another
    Implements, // impl implements trait
    TypeRef,    // references a type
    DependsOn,  // generic dependency
    Notifies,   // ansible: task notifies handler
    References, // references a variable/symbol
    LinksTo,    // docs: links to another document
    Includes,   // includes/imports another file
}

/// The core graph type
pub type CodeGraph = StableGraph<Node, EdgeKind>;

impl Node {
    pub fn new(
        kind: NodeKind,
        name: impl Into<String>,
        file: impl Into<PathBuf>,
        span: Span,
    ) -> Self {
        Self {
            kind,
            name: name.into(),
            file: file.into(),
            span,
            signature: None,
            visibility: Visibility::Private,
        }
    }

    #[must_use]
    pub fn with_signature(mut self, sig: impl Into<String>) -> Self {
        self.signature = Some(sig.into());
        self
    }

    #[must_use]
    pub const fn with_visibility(mut self, vis: Visibility) -> Self {
        self.visibility = vis;
        self
    }
}

impl Span {
    #[must_use]
    pub const fn new(start_line: u32, end_line: u32, start_col: u32, end_col: u32) -> Self {
        Self {
            start_line,
            end_line,
            start_col,
            end_col,
        }
    }

    #[must_use]
    pub const fn lines(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }
}
