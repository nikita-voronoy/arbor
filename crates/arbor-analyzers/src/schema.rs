use anyhow::Result;
use arbor_core::graph::{EdgeKind, Node, NodeKind, Span, Visibility};
use arbor_core::palace::Palace;
use arbor_detect::ProjectFacet;
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::Analyzer;

/// Analyzer for schema files: SQL migrations, protobuf, `OpenAPI`
pub struct SchemaAnalyzer {
    // SQL patterns
    create_table_re: Regex,
    column_re: Regex,
    fk_re: Regex,
}

impl SchemaAnalyzer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            create_table_re: Regex::new(r"(?i)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)")?,
            column_re: Regex::new(
                r"(?i)^\s+(\w+)\s+(INTEGER|TEXT|VARCHAR|BOOLEAN|TIMESTAMP|UUID|BIGINT|SERIAL|INT|REAL|FLOAT|DOUBLE|DECIMAL|CHAR|BLOB|DATE|TIME|JSON|JSONB)",
            )?,
            fk_re: Regex::new(r"(?i)REFERENCES\s+(\w+)\s*\((\w+)\)")?,
        })
    }

    fn parse_sql(&self, path: &Path, palace: &mut Palace) -> Result<()> {
        let content = std::fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();

        let file_node = Node::new(
            NodeKind::File,
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"),
            path,
            Span::new(1, lines.len() as u32, 0, 0),
        );
        let file_idx = palace.add_node(file_node);

        let mut current_table: Option<(petgraph::stable_graph::NodeIndex, String)> = None;

        for (i, line) in lines.iter().enumerate() {
            let line_num = i as u32 + 1;

            // Check for CREATE TABLE
            if let Some(cap) = self.create_table_re.captures(line) {
                let table_name = cap[1].to_string();
                let table_node = Node::new(
                    NodeKind::Table,
                    &table_name,
                    path,
                    Span::new(line_num, line_num, 0, 0),
                )
                .with_visibility(Visibility::Public)
                .with_signature(format!("CREATE TABLE {table_name}"));
                let table_idx = palace.add_node(table_node);
                palace.add_edge(file_idx, table_idx, EdgeKind::Contains);
                current_table = Some((table_idx, table_name.clone()));
                continue;
            }

            // Check for columns (only within a CREATE TABLE)
            if let Some((table_idx, _)) = &current_table {
                if let Some(cap) = self.column_re.captures(line) {
                    let col_name = cap[1].to_string();
                    let col_type = cap[2].to_string();
                    let col_node = Node::new(
                        NodeKind::Column,
                        &col_name,
                        path,
                        Span::new(line_num, line_num, 0, 0),
                    )
                    .with_signature(format!("{col_name} {col_type}"));
                    let col_idx = palace.add_node(col_node);
                    palace.add_edge(*table_idx, col_idx, EdgeKind::Contains);

                    // Check for foreign key in the same line
                    if let Some(fk_cap) = self.fk_re.captures(line) {
                        let ref_table = fk_cap[1].to_string();
                        let targets = palace.find_by_name(&ref_table).to_vec();
                        for target_idx in targets {
                            palace.add_edge(*table_idx, target_idx, EdgeKind::References);
                        }
                    }
                }

                // End of CREATE TABLE
                if line.contains(");") {
                    current_table = None;
                }
            }

            // Standalone REFERENCES / FOREIGN KEY
            if let Some(fk_cap) = self.fk_re.captures(line)
                && let Some((table_idx, _)) = &current_table
            {
                let ref_table = fk_cap[1].to_string();
                let targets = palace.find_by_name(&ref_table).to_vec();
                for target_idx in targets {
                    palace.add_edge(*table_idx, target_idx, EdgeKind::References);
                }
            }
        }

        Ok(())
    }

    fn parse_openapi(&self, path: &Path, palace: &mut Palace) -> Result<()> {
        let content = std::fs::read_to_string(path)?;
        let doc: serde_yaml::Value = serde_yaml::from_str(&content)?;

        let file_node = Node::new(
            NodeKind::File,
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"),
            path,
            Span::new(1, content.lines().count() as u32, 0, 0),
        )
        .with_visibility(Visibility::Public);
        let file_idx = palace.add_node(file_node);

        // Extract paths/endpoints
        if let Some(paths) = doc.get("paths").and_then(|v| v.as_mapping()) {
            for (path_key, methods) in paths {
                let endpoint = path_key.as_str().unwrap_or("");
                if let Some(methods_map) = methods.as_mapping() {
                    for (method, _) in methods_map {
                        let method_str = method.as_str().unwrap_or("");
                        let name = format!("{} {}", method_str.to_uppercase(), endpoint);
                        let endpoint_node =
                            Node::new(NodeKind::Endpoint, &name, path, Span::new(1, 1, 0, 0))
                                .with_visibility(Visibility::Public)
                                .with_signature(name.clone());
                        let ep_idx = palace.add_node(endpoint_node);
                        palace.add_edge(file_idx, ep_idx, EdgeKind::Contains);
                    }
                }
            }
        }

        // Extract schemas/definitions
        let schemas = doc
            .get("components")
            .and_then(|v| v.get("schemas"))
            .or_else(|| doc.get("definitions"));

        if let Some(schemas_map) = schemas.and_then(|v| v.as_mapping()) {
            for (schema_name, _) in schemas_map {
                let name = schema_name.as_str().unwrap_or("");
                let schema_node = Node::new(NodeKind::Message, name, path, Span::new(1, 1, 0, 0))
                    .with_visibility(Visibility::Public);
                let schema_idx = palace.add_node(schema_node);
                palace.add_edge(file_idx, schema_idx, EdgeKind::Contains);
            }
        }

        Ok(())
    }

    fn parse_protobuf(&self, path: &Path, palace: &mut Palace) -> Result<()> {
        let content = std::fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();

        let file_node = Node::new(
            NodeKind::File,
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"),
            path,
            Span::new(1, lines.len() as u32, 0, 0),
        );
        let file_idx = palace.add_node(file_node);

        let message_re = Regex::new(r"^\s*message\s+(\w+)")?;
        let service_re = Regex::new(r"^\s*service\s+(\w+)")?;
        let rpc_re = Regex::new(r"^\s*rpc\s+(\w+)\s*\((\w+)\)\s*returns\s*\((\w+)\)")?;

        let mut current_parent = file_idx;

        for (i, line) in lines.iter().enumerate() {
            let line_num = i as u32 + 1;

            if let Some(cap) = message_re.captures(line) {
                let msg_node = Node::new(
                    NodeKind::Message,
                    &cap[1],
                    path,
                    Span::new(line_num, line_num, 0, 0),
                )
                .with_visibility(Visibility::Public)
                .with_signature(format!("message {}", &cap[1]));
                let msg_idx = palace.add_node(msg_node);
                palace.add_edge(file_idx, msg_idx, EdgeKind::Contains);
                current_parent = msg_idx;
            }

            if let Some(cap) = service_re.captures(line) {
                let svc_node = Node::new(
                    NodeKind::Trait,
                    &cap[1],
                    path,
                    Span::new(line_num, line_num, 0, 0),
                )
                .with_visibility(Visibility::Public)
                .with_signature(format!("service {}", &cap[1]));
                let svc_idx = palace.add_node(svc_node);
                palace.add_edge(file_idx, svc_idx, EdgeKind::Contains);
                current_parent = svc_idx;
            }

            if let Some(cap) = rpc_re.captures(line) {
                let rpc_node = Node::new(
                    NodeKind::Function,
                    &cap[1],
                    path,
                    Span::new(line_num, line_num, 0, 0),
                )
                .with_visibility(Visibility::Public)
                .with_signature(format!(
                    "rpc {}({}) returns ({})",
                    &cap[1], &cap[2], &cap[3]
                ));
                let rpc_idx = palace.add_node(rpc_node);
                palace.add_edge(current_parent, rpc_idx, EdgeKind::Contains);

                // Reference input/output message types
                for type_name in [&cap[2], &cap[3]] {
                    let targets = palace.find_by_name(type_name).to_vec();
                    for target_idx in targets {
                        palace.add_edge(rpc_idx, target_idx, EdgeKind::TypeRef);
                    }
                }
            }

            if line.trim() == "}" {
                current_parent = file_idx;
            }
        }

        Ok(())
    }
}

impl Analyzer for SchemaAnalyzer {
    fn can_handle(&self, _facet: &ProjectFacet) -> bool {
        // Schema analyzer is always active — it picks up .sql, .proto, openapi.yml files
        true
    }

    fn analyze(&self, root: &Path, palace: &mut Palace) -> Result<()> {
        self.walk_and_parse(root, palace)
    }

    fn analyze_file(&self, path: &Path, _source: &str, palace: &mut Palace) -> Result<()> {
        self.dispatch_file(path, palace)
    }
}

impl SchemaAnalyzer {
    fn walk_and_parse(&self, dir: &Path, palace: &mut Palace) -> Result<()> {
        let files: Vec<PathBuf> = ignore::WalkBuilder::new(dir)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
            .map(ignore::DirEntry::into_path)
            .collect();

        for path in files {
            self.dispatch_file(&path, palace)?;
        }

        Ok(())
    }

    fn dispatch_file(&self, path: &Path, palace: &mut Palace) -> Result<()> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        match ext {
            "sql" => self.parse_sql(path, palace)?,
            "proto" => self.parse_protobuf(path, palace)?,
            "yml" | "yaml" if name.contains("openapi") || name.contains("swagger") => {
                self.parse_openapi(path, palace)?;
            }
            "json" if name.contains("openapi") || name.contains("swagger") => {
                // JSON OpenAPI — would need serde_json, skip for now
            }
            _ => {}
        }

        Ok(())
    }
}
