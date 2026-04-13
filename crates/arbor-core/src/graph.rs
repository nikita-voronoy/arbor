use petgraph::stable_graph::StableGraph;
use serde::{Deserialize, Serialize};
use std::fmt;
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

impl NodeKind {
    /// Full label for skeleton display (e.g. "fn", "struct", "trait")
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Module => "mod",
            Self::Function => "fn",
            Self::Struct => "struct",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::Enum => "enum",
            Self::EnumVariant => "variant",
            Self::Constant => "const",
            Self::TypeAlias => "type",
            Self::Macro => "macro",
            Self::Role => "role",
            Self::Task => "task",
            Self::Handler => "handler",
            Self::Variable => "var",
            Self::Template => "tpl",
            Self::Resource => "res",
            Self::Document => "doc",
            Self::Section => "sec",
            Self::CodeBlock => "code",
            Self::Table => "tbl",
            Self::Column => "col",
            Self::Endpoint => "ep",
            Self::Message => "msg",
        }
    }

    /// Short tag for compact skeleton display (e.g. "fn", "st", "tr")
    #[must_use]
    pub const fn short_tag(self) -> &'static str {
        match self {
            Self::Function => "fn",
            Self::Struct => "st",
            Self::Trait => "tr",
            Self::Enum => "en",
            Self::Module => "mod",
            Self::Constant => "co",
            Self::Macro => "def",
            Self::TypeAlias => "ty",
            Self::Role => "role",
            Self::Task => "task",
            Self::Handler => "hnd",
            Self::Variable => "var",
            Self::Template => "tpl",
            Self::Resource => "res",
            Self::Document => "doc",
            Self::Section => "sec",
            Self::CodeBlock => "code",
            Self::Table => "tbl",
            Self::Endpoint => "ep",
            Self::Message => "msg",
            Self::EnumVariant | Self::Column | Self::Impl | Self::File => "",
        }
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
