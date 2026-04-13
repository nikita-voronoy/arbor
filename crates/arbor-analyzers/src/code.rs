use anyhow::{Context, Result};
use arbor_core::graph::{EdgeKind, Node, NodeKind, Span, Visibility};
use arbor_core::palace::Palace;
use arbor_detect::ProjectFacet;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Parser, Tree};

use crate::Analyzer;

pub struct CodeAnalyzer {
    languages: HashMap<&'static str, LanguageConfig>,
}

struct LanguageConfig {
    language: Language,
    extensions: &'static [&'static str],
    queries: NodeQueries,
}

/// Tree-sitter node type names for extracting code structure
struct NodeQueries {
    function: &'static [&'static str],
    struct_like: &'static [&'static str],
    trait_like: &'static [&'static str],
    impl_block: &'static [&'static str],
    enum_def: &'static [&'static str],
    enum_variant: &'static [&'static str],
    macro_def: &'static [&'static str],
    use_decl: &'static [&'static str],
    call_expr: &'static [&'static str],
}

impl CodeAnalyzer {
    #[must_use]
    pub fn new() -> Self {
        let mut languages = HashMap::new();

        languages.insert(
            "rust",
            LanguageConfig {
                language: tree_sitter_rust::LANGUAGE.into(),
                extensions: &["rs"],
                queries: NodeQueries {
                    function: &["function_item"],
                    struct_like: &["struct_item"],
                    trait_like: &["trait_item"],
                    impl_block: &["impl_item"],
                    enum_def: &["enum_item"],
                    enum_variant: &["enum_variant"],
                    macro_def: &["macro_definition"],
                    use_decl: &["use_declaration"],
                    call_expr: &["call_expression"],
                },
            },
        );

        languages.insert(
            "python",
            LanguageConfig {
                language: tree_sitter_python::LANGUAGE.into(),
                extensions: &["py"],
                queries: NodeQueries {
                    function: &["function_definition"],
                    struct_like: &["class_definition"],
                    trait_like: &[],
                    impl_block: &[],
                    enum_def: &[],
                    enum_variant: &[],
                    macro_def: &[],
                    use_decl: &["import_statement", "import_from_statement"],
                    call_expr: &["call"],
                },
            },
        );

        languages.insert(
            "typescript",
            LanguageConfig {
                language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                extensions: &["ts"],
                queries: NodeQueries {
                    function: &[
                        "function_declaration",
                        "method_definition",
                        "arrow_function",
                    ],
                    struct_like: &["class_declaration"],
                    trait_like: &["interface_declaration"],
                    impl_block: &[],
                    enum_def: &["enum_declaration"],
                    enum_variant: &["property_identifier"],
                    macro_def: &[],
                    use_decl: &["import_statement"],
                    call_expr: &["call_expression"],
                },
            },
        );

        languages.insert(
            "tsx",
            LanguageConfig {
                language: tree_sitter_typescript::LANGUAGE_TSX.into(),
                extensions: &["tsx", "jsx"],
                queries: NodeQueries {
                    function: &[
                        "function_declaration",
                        "method_definition",
                        "arrow_function",
                    ],
                    struct_like: &["class_declaration"],
                    trait_like: &["interface_declaration"],
                    impl_block: &[],
                    enum_def: &["enum_declaration"],
                    enum_variant: &[],
                    macro_def: &[],
                    use_decl: &["import_statement"],
                    call_expr: &["call_expression"],
                },
            },
        );

        languages.insert(
            "javascript",
            LanguageConfig {
                language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                extensions: &["js", "mjs", "cjs"],
                queries: NodeQueries {
                    function: &[
                        "function_declaration",
                        "method_definition",
                        "arrow_function",
                    ],
                    struct_like: &["class_declaration"],
                    trait_like: &[],
                    impl_block: &[],
                    enum_def: &[],
                    enum_variant: &[],
                    macro_def: &[],
                    use_decl: &["import_statement"],
                    call_expr: &["call_expression"],
                },
            },
        );

        languages.insert(
            "go",
            LanguageConfig {
                language: tree_sitter_go::LANGUAGE.into(),
                extensions: &["go"],
                queries: NodeQueries {
                    function: &["function_declaration", "method_declaration"],
                    struct_like: &["type_declaration"],
                    trait_like: &[],
                    impl_block: &[],
                    enum_def: &[],
                    enum_variant: &[],
                    macro_def: &[],
                    use_decl: &["import_declaration"],
                    call_expr: &["call_expression"],
                },
            },
        );

        languages.insert(
            "c",
            LanguageConfig {
                language: tree_sitter_c::LANGUAGE.into(),
                extensions: &["c", "h"],
                queries: NodeQueries {
                    function: &["function_definition"],
                    struct_like: &["struct_specifier"],
                    trait_like: &[],
                    impl_block: &[],
                    enum_def: &["enum_specifier"],
                    enum_variant: &["enumerator"],
                    macro_def: &["preproc_def", "preproc_function_def"],
                    use_decl: &["preproc_include"],
                    call_expr: &["call_expression"],
                },
            },
        );

        languages.insert(
            "cpp",
            LanguageConfig {
                language: tree_sitter_cpp::LANGUAGE.into(),
                extensions: &["cpp", "cc", "cxx", "hpp", "hxx"],
                queries: NodeQueries {
                    function: &["function_definition"],
                    struct_like: &["struct_specifier", "class_specifier"],
                    trait_like: &[],
                    impl_block: &[],
                    enum_def: &["enum_specifier"],
                    enum_variant: &["enumerator"],
                    macro_def: &["preproc_def", "preproc_function_def"],
                    use_decl: &["preproc_include", "using_declaration"],
                    call_expr: &["call_expression"],
                },
            },
        );

        languages.insert(
            "csharp",
            LanguageConfig {
                language: tree_sitter_c_sharp::LANGUAGE.into(),
                extensions: &["cs"],
                queries: NodeQueries {
                    function: &[
                        "method_declaration",
                        "constructor_declaration",
                        "local_function_statement",
                    ],
                    struct_like: &[
                        "class_declaration",
                        "struct_declaration",
                        "record_declaration",
                    ],
                    trait_like: &["interface_declaration"],
                    impl_block: &[],
                    enum_def: &["enum_declaration"],
                    enum_variant: &["enum_member_declaration"],
                    macro_def: &[],
                    use_decl: &["using_directive"],
                    call_expr: &["invocation_expression", "object_creation_expression"],
                },
            },
        );

        Self { languages }
    }

    fn language_for_file(&self, path: &Path) -> Option<(&str, &LanguageConfig)> {
        let ext = path.extension()?.to_str()?;
        self.languages
            .iter()
            .find(|(_, config)| config.extensions.contains(&ext))
            .map(|(&name, config)| (name, config))
    }

    fn walk_files(&self, root: &Path) -> Vec<PathBuf> {
        ignore::WalkBuilder::new(root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
            .map(ignore::DirEntry::into_path)
            .filter(|path| self.language_for_file(path).is_some())
            .collect()
    }

    fn parse_file(_path: &Path, source: &str, config: &LanguageConfig) -> Result<Tree> {
        let mut parser = Parser::new();
        parser
            .set_language(&config.language)
            .context("Failed to set language")?;
        parser.parse(source, None).context("Failed to parse file")
    }

    fn extract_nodes(
        &self,
        tree: &Tree,
        source: &[u8],
        file_path: &Path,
        config: &LanguageConfig,
        palace: &mut Palace,
    ) {
        let root = tree.root_node();
        // Create a file node
        let file_node = Node::new(
            NodeKind::File,
            file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"),
            file_path,
            Span::new(
                root.start_position().row as u32,
                root.end_position().row as u32,
                0,
                0,
            ),
        )
        .with_visibility(Visibility::Public);
        let file_idx = palace.add_node(file_node);

        // Walk the tree and extract nodes
        let mut cursor = root.walk();
        self.walk_tree(&mut cursor, source, file_path, file_idx, config, palace);
    }

    fn walk_tree(
        &self,
        cursor: &mut tree_sitter::TreeCursor,
        source: &[u8],
        file_path: &Path,
        parent_idx: petgraph::stable_graph::NodeIndex,
        config: &LanguageConfig,
        palace: &mut Palace,
    ) {
        let ts_node = cursor.node();
        let kind = ts_node.kind();

        let node_kind = if config.queries.function.contains(&kind) {
            Some(NodeKind::Function)
        } else if config.queries.struct_like.contains(&kind) {
            // C/C++: only index struct/class definitions (with a body), not forward decls or type references
            // struct foo { ... } → has field_declaration_list or declaration_list → definition
            // struct foo *bar   → no body → skip (type reference, not definition)
            let has_body = ts_node.child_by_field_name("body").is_some()
                || (0..ts_node.child_count()).any(|i| {
                    ts_node.child(i).is_some_and(|c| {
                        let ck = c.kind();
                        ck == "field_declaration_list" || ck == "declaration_list"
                    })
                });
            if has_body {
                Some(NodeKind::Struct)
            } else {
                // Not a definition — record a TypeRef edge to the real definition
                let ref_name = Self::extract_name(&ts_node, source);
                if ref_name != "anonymous" {
                    let targets = palace.find_by_name(&ref_name).to_vec();
                    for target_idx in targets {
                        if let Some(tn) = palace.get_node(target_idx)
                            && matches!(tn.kind, NodeKind::Struct)
                        {
                            palace.add_edge(parent_idx, target_idx, EdgeKind::TypeRef);
                            break;
                        }
                    }
                }
                None
            }
        } else if config.queries.trait_like.contains(&kind) {
            Some(NodeKind::Trait)
        } else if config.queries.impl_block.contains(&kind) {
            Some(NodeKind::Impl)
        } else if config.queries.enum_def.contains(&kind) {
            // Same logic: only index enum definitions with a body
            let has_body = ts_node.child_by_field_name("body").is_some()
                || (0..ts_node.child_count()).any(|i| {
                    ts_node
                        .child(i)
                        .is_some_and(|c| c.kind() == "enumerator_list")
                });
            if has_body {
                Some(NodeKind::Enum)
            } else {
                let ref_name = Self::extract_name(&ts_node, source);
                if ref_name != "anonymous" {
                    let targets = palace.find_by_name(&ref_name).to_vec();
                    for target_idx in targets {
                        if let Some(tn) = palace.get_node(target_idx)
                            && matches!(tn.kind, NodeKind::Enum)
                        {
                            palace.add_edge(parent_idx, target_idx, EdgeKind::TypeRef);
                            break;
                        }
                    }
                }
                None
            }
        } else if config.queries.enum_variant.contains(&kind) {
            Some(NodeKind::EnumVariant)
        } else if config.queries.macro_def.contains(&kind) {
            Some(NodeKind::Macro)
        } else {
            None
        };

        let current_parent = if let Some(nk) = node_kind {
            let name = Self::extract_name(&ts_node, source);
            let sig = Self::extract_signature(&ts_node, source);
            let vis = Self::extract_visibility(&ts_node, source);

            let span = Span::new(
                ts_node.start_position().row as u32 + 1,
                ts_node.end_position().row as u32 + 1,
                ts_node.start_position().column as u32,
                ts_node.end_position().column as u32,
            );

            let mut node = Node::new(nk, &name, file_path, span).with_visibility(vis);

            // For macros: extract the #define name and value as signature
            if matches!(nk, NodeKind::Macro) {
                let macro_sig = Self::extract_macro_signature(&ts_node, source);
                node = node.with_signature(macro_sig);
            } else if let Some(s) = sig {
                node = node.with_signature(s);
            }

            // Skip anonymous nodes for non-function types (reduces noise)
            if name == "anonymous" && !matches!(nk, NodeKind::Function) {
                parent_idx
            } else {
                let idx = palace.add_node(node);
                palace.add_edge(parent_idx, idx, EdgeKind::Contains);

                // Extract call edges from function bodies
                if matches!(nk, NodeKind::Function) {
                    Self::extract_calls(&ts_node, source, idx, config, palace);
                }

                idx
            }
        } else if config.queries.use_decl.contains(&kind) {
            // Extract import edges
            Self::extract_import(&ts_node, source, parent_idx, palace);
            parent_idx
        } else {
            parent_idx
        };

        // Recurse into children (with stack growth for deeply nested ASTs)
        if cursor.goto_first_child() {
            loop {
                stacker::maybe_grow(64 * 1024, 2 * 1024 * 1024, || {
                    self.walk_tree(cursor, source, file_path, current_parent, config, palace);
                });
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    fn extract_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let kind = node.kind();

        // C/C++: function_definition → declarator (function_declarator) → declarator (identifier)
        if kind == "function_definition"
            && let Some(declarator) = node.child_by_field_name("declarator")
        {
            return Self::extract_declarator_name(&declarator, source);
        }

        // C/C++: struct_specifier / class_specifier / enum_specifier → name
        if (kind == "struct_specifier" || kind == "class_specifier" || kind == "enum_specifier")
            && let Some(name_node) = node.child_by_field_name("name")
        {
            return name_node
                .utf8_text(source)
                .unwrap_or("anonymous")
                .to_string();
        }

        // Python: function_definition / class_definition → name field
        if (kind == "function_definition" || kind == "class_definition")
            && let Some(name_node) = node.child_by_field_name("name")
        {
            return name_node
                .utf8_text(source)
                .unwrap_or("anonymous")
                .to_string();
        }

        // TS/JS: function_declaration, class_declaration → name field
        if let Some(name_node) = node.child_by_field_name("name") {
            return name_node
                .utf8_text(source)
                .unwrap_or("anonymous")
                .to_string();
        }

        // Fallback: first identifier/type_identifier child
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                let ck = child.kind();
                if ck == "identifier" || ck == "type_identifier" || ck == "field_identifier" {
                    return child.utf8_text(source).unwrap_or("unknown").to_string();
                }
            }
        }
        "anonymous".to_string()
    }

    /// Recursively extract the identifier name from a C/C++ declarator chain
    /// e.g. `function_declarator` → `pointer_declarator` → identifier
    fn extract_declarator_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let kind = node.kind();

        if kind == "identifier" {
            return node.utf8_text(source).unwrap_or("anonymous").to_string();
        }

        // function_declarator → declarator field holds the name (or another wrapper)
        if let Some(inner) = node.child_by_field_name("declarator") {
            return Self::extract_declarator_name(&inner, source);
        }

        // Fallback: first identifier anywhere in children
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i)
                && child.kind() == "identifier"
            {
                return child.utf8_text(source).unwrap_or("anonymous").to_string();
            }
        }

        "anonymous".to_string()
    }

    fn extract_signature(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let kind = node.kind();

        // Rust: function_item
        if kind == "function_item" {
            let start = node.start_byte();
            let end = node
                .child_by_field_name("body")
                .map_or_else(|| node.end_byte(), |b| b.start_byte());
            let sig = std::str::from_utf8(&source[start..end])
                .unwrap_or("")
                .trim()
                .to_string();
            return Some(sig);
        }

        // C/C++: function_definition → everything before compound_statement body
        if kind == "function_definition" {
            let start = node.start_byte();
            let end = node
                .child_by_field_name("body")
                .map_or_else(|| node.end_byte(), |b| b.start_byte());
            let sig = std::str::from_utf8(&source[start..end])
                .unwrap_or("")
                .trim();
            // Truncate long C signatures
            if sig.len() > 200 {
                return Some(format!("{}...", &sig[..200]));
            }
            return Some(sig.to_string());
        }

        // C#: method_declaration, constructor_declaration → everything before body
        if kind == "method_declaration" || kind == "constructor_declaration" {
            let start = node.start_byte();
            let end = node
                .child_by_field_name("body")
                .map_or_else(|| node.end_byte(), |b| b.start_byte());
            let sig = std::str::from_utf8(&source[start..end])
                .unwrap_or("")
                .trim();
            if sig.len() > 200 {
                return Some(format!("{}...", &sig[..200]));
            }
            return Some(sig.to_string());
        }

        None
    }

    /// Extract macro signature: #define NAME VALUE or #define NAME(args) VALUE
    fn extract_macro_signature(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = node.utf8_text(source).unwrap_or("");
        // Take first line only, truncate if too long
        let first_line = text.lines().next().unwrap_or(text);
        if first_line.len() > 120 {
            format!("{}...", &first_line[..120])
        } else {
            first_line.to_string()
        }
    }

    fn extract_visibility(node: &tree_sitter::Node, source: &[u8]) -> Visibility {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                let ck = child.kind();
                // Rust: pub
                if ck == "visibility_modifier" {
                    let text = child.utf8_text(source).unwrap_or("");
                    if text.contains("pub") {
                        if text.contains("crate") {
                            return Visibility::Crate;
                        }
                        return Visibility::Public;
                    }
                }
                // C/C++: static = file-private, no static = public
                if ck == "storage_class_specifier" {
                    let text = child.utf8_text(source).unwrap_or("");
                    if text == "static" {
                        return Visibility::Private;
                    }
                }
                // TS/JS: export = public
                if ck == "export" || ck == "export_statement" {
                    return Visibility::Public;
                }
                // C#: modifier keywords
                if ck == "modifier" {
                    let text = child.utf8_text(source).unwrap_or("");
                    if text == "public" {
                        return Visibility::Public;
                    }
                    if text == "internal" {
                        return Visibility::Crate;
                    }
                    if text == "private" || text == "protected" {
                        return Visibility::Private;
                    }
                }
            }
        }
        // C default: non-static functions are public (visible across TUs)
        let kind = node.kind();
        if kind == "function_definition" || kind == "struct_specifier" || kind == "enum_specifier" {
            return Visibility::Public;
        }
        Visibility::Private
    }

    fn extract_calls(
        node: &tree_sitter::Node,
        source: &[u8],
        fn_idx: petgraph::stable_graph::NodeIndex,
        config: &LanguageConfig,
        palace: &mut Palace,
    ) {
        let mut cursor = node.walk();
        Self::find_calls_recursive(&mut cursor, source, fn_idx, config, palace);
    }

    fn find_calls_recursive(
        cursor: &mut tree_sitter::TreeCursor,
        source: &[u8],
        fn_idx: petgraph::stable_graph::NodeIndex,
        config: &LanguageConfig,
        palace: &mut Palace,
    ) {
        let node = cursor.node();

        if config.queries.call_expr.contains(&node.kind()) {
            // Extract the function name from the call
            if let Some(func_node) = node.child_by_field_name("function") {
                let call_name = func_node.utf8_text(source).unwrap_or("").to_string();
                // Extract just the last segment (e.g., "foo::bar" → "bar")
                let short_name = call_name.rsplit("::").next().unwrap_or(&call_name);

                // Try to find the target in the graph
                let targets: Vec<_> = palace.find_by_name(short_name).to_vec();
                if targets.is_empty() {
                    // Target not yet indexed — defer to second pass
                    palace.add_pending_call(fn_idx, short_name.to_string());
                } else {
                    for target in targets {
                        if target != fn_idx {
                            palace.add_edge(fn_idx, target, EdgeKind::Calls);
                        }
                    }
                }
            }
        }

        if cursor.goto_first_child() {
            loop {
                stacker::maybe_grow(64 * 1024, 2 * 1024 * 1024, || {
                    Self::find_calls_recursive(cursor, source, fn_idx, config, palace);
                });
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    fn extract_import(
        node: &tree_sitter::Node,
        source: &[u8],
        parent_idx: petgraph::stable_graph::NodeIndex,
        palace: &mut Palace,
    ) {
        let import_text = node.utf8_text(source).unwrap_or("").to_string();
        // For now just record the import as a reference — full resolution comes later
        let _ = (import_text, parent_idx, palace);
    }
}

impl Analyzer for CodeAnalyzer {
    fn can_handle(&self, facet: &ProjectFacet) -> bool {
        matches!(
            facet,
            ProjectFacet::Rust
                | ProjectFacet::Python
                | ProjectFacet::TypeScript
                | ProjectFacet::JavaScript
                | ProjectFacet::Go
                | ProjectFacet::C
                | ProjectFacet::Cpp
                | ProjectFacet::CSharp
        )
    }

    fn analyze(&self, root: &Path, palace: &mut Palace) -> Result<()> {
        use rayon::prelude::*;

        let files = self.walk_files(root);

        // Phase 1: read + parse in parallel (I/O + CPU bound)
        let parsed: Vec<_> = files
            .par_iter()
            .filter_map(|file| {
                let source = std::fs::read_to_string(file).ok()?;
                let (_, config) = self.language_for_file(file)?;
                let mut parser = Parser::new();
                parser.set_language(&config.language).ok()?;
                let tree = parser.parse(&source, None)?;
                Some((file.clone(), source, tree))
            })
            .collect();

        // Phase 2: extract nodes sequentially (mutates Palace)
        for (file, source, tree) in &parsed {
            let (_, config) = self.language_for_file(file).unwrap();
            self.extract_nodes(tree, source.as_bytes(), file, config, palace);
        }

        Ok(())
    }

    fn analyze_file(&self, path: &Path, source: &str, palace: &mut Palace) -> Result<()> {
        let (_lang_name, config) = self
            .language_for_file(path)
            .context("Unsupported file type")?;

        let tree = Self::parse_file(path, source, config)?;
        self.extract_nodes(&tree, source.as_bytes(), path, config, palace);
        Ok(())
    }
}
