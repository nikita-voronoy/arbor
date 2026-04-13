use anyhow::{Context, Result};
use arbor_core::graph::{EdgeKind, Node, NodeKind, Span, Visibility};
use arbor_core::palace::Palace;
use arbor_detect::ProjectFacet;
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::Analyzer;

// ========================
// Ansible Analyzer
// ========================

pub struct AnsibleAnalyzer {
    jinja_var_re: Regex,
}

impl AnsibleAnalyzer {
    /// # Panics
    /// Panics if the built-in Jinja variable regex is invalid (should never happen).
    #[must_use]
    pub fn new() -> Self {
        Self {
            jinja_var_re: Regex::new(r"\{\{\s*(\w+)\s*\}\}").unwrap(),
        }
    }

    fn analyze_roles(&self, root: &Path, palace: &mut Palace) -> Result<()> {
        let roles_dir = root.join("roles");
        if !roles_dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&roles_dir)?.filter_map(std::result::Result::ok) {
            let role_path = entry.path();
            if !role_path.is_dir() {
                continue;
            }

            let role_name = role_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let role_node = Node::new(NodeKind::Role, role_name, &role_path, Span::new(1, 1, 0, 0))
                .with_visibility(Visibility::Public);
            let role_idx = palace.add_node(role_node);

            // Parse tasks
            self.analyze_tasks(&role_path.join("tasks"), role_idx, palace)?;
            // Parse handlers
            self.analyze_handlers(&role_path.join("handlers"), role_idx, palace)?;
            // Parse defaults (variables)
            self.analyze_defaults(&role_path.join("defaults"), role_idx, palace)?;
            // Parse templates
            self.analyze_templates(&role_path.join("templates"), role_idx, palace)?;
        }

        Ok(())
    }

    fn analyze_tasks(
        &self,
        tasks_dir: &Path,
        role_idx: petgraph::stable_graph::NodeIndex,
        palace: &mut Palace,
    ) -> Result<()> {
        let main_yml = tasks_dir.join("main.yml");
        if !main_yml.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&main_yml)?;
        let docs: Vec<serde_yaml::Value> = serde_yaml::from_str(&content).unwrap_or_default();

        for (i, task) in docs.iter().enumerate() {
            if let Some(name) = task.get("name").and_then(|v| v.as_str()) {
                let task_node = Node::new(
                    NodeKind::Task,
                    name,
                    &main_yml,
                    Span::new(i as u32 + 1, i as u32 + 1, 0, 0),
                );
                let task_idx = palace.add_node(task_node);
                palace.add_edge(role_idx, task_idx, EdgeKind::Contains);

                // Check for notify -> handler reference
                if let Some(notify) = task.get("notify").and_then(|v| v.as_str()) {
                    let handler_nodes = palace.find_by_name(notify).to_vec();
                    for handler_idx in handler_nodes {
                        palace.add_edge(task_idx, handler_idx, EdgeKind::Notifies);
                    }
                }

                // Extract Jinja2 variable references from the task
                let task_str = serde_yaml::to_string(task).unwrap_or_default();
                for cap in self.jinja_var_re.captures_iter(&task_str) {
                    let var_name = &cap[1];
                    let var_nodes = palace.find_by_name(var_name).to_vec();
                    for var_idx in var_nodes {
                        palace.add_edge(task_idx, var_idx, EdgeKind::References);
                    }
                }

                // Check for include_role / import_tasks
                if let Some(include) = task.get("include_role").or_else(|| task.get("import_role"))
                    && let Some(role_name) = include.get("name").and_then(|v| v.as_str())
                {
                    let target_roles = palace.find_by_name(role_name).to_vec();
                    for target_idx in target_roles {
                        palace.add_edge(task_idx, target_idx, EdgeKind::Includes);
                    }
                }
            }
        }

        Ok(())
    }

    fn analyze_handlers(
        &self,
        handlers_dir: &Path,
        role_idx: petgraph::stable_graph::NodeIndex,
        palace: &mut Palace,
    ) -> Result<()> {
        let main_yml = handlers_dir.join("main.yml");
        if !main_yml.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&main_yml)?;
        let docs: Vec<serde_yaml::Value> = serde_yaml::from_str(&content).unwrap_or_default();

        for (i, handler) in docs.iter().enumerate() {
            if let Some(name) = handler.get("name").and_then(|v| v.as_str()) {
                let handler_node = Node::new(
                    NodeKind::Handler,
                    name,
                    &main_yml,
                    Span::new(i as u32 + 1, i as u32 + 1, 0, 0),
                );
                let handler_idx = palace.add_node(handler_node);
                palace.add_edge(role_idx, handler_idx, EdgeKind::Contains);
            }
        }

        Ok(())
    }

    fn analyze_defaults(
        &self,
        defaults_dir: &Path,
        role_idx: petgraph::stable_graph::NodeIndex,
        palace: &mut Palace,
    ) -> Result<()> {
        let main_yml = defaults_dir.join("main.yml");
        if !main_yml.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&main_yml)?;
        let vars: serde_yaml::Value =
            serde_yaml::from_str(&content).unwrap_or(serde_yaml::Value::Null);

        if let serde_yaml::Value::Mapping(map) = vars {
            for (i, (key, _)) in map.iter().enumerate() {
                if let Some(var_name) = key.as_str() {
                    let var_node = Node::new(
                        NodeKind::Variable,
                        var_name,
                        &main_yml,
                        Span::new(i as u32 + 1, i as u32 + 1, 0, 0),
                    );
                    let var_idx = palace.add_node(var_node);
                    palace.add_edge(role_idx, var_idx, EdgeKind::Contains);
                }
            }
        }

        Ok(())
    }

    fn analyze_templates(
        &self,
        templates_dir: &Path,
        role_idx: petgraph::stable_graph::NodeIndex,
        palace: &mut Palace,
    ) -> Result<()> {
        if !templates_dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(templates_dir)?.filter_map(std::result::Result::ok) {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "j2" {
                continue;
            }

            let tmpl_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let tmpl_node = Node::new(NodeKind::Template, tmpl_name, &path, Span::new(1, 1, 0, 0));
            let tmpl_idx = palace.add_node(tmpl_node);
            palace.add_edge(role_idx, tmpl_idx, EdgeKind::Contains);

            // Extract variable references from template
            if let Ok(content) = std::fs::read_to_string(&path) {
                for cap in self.jinja_var_re.captures_iter(&content) {
                    let var_name = &cap[1];
                    let var_nodes = palace.find_by_name(var_name).to_vec();
                    for var_idx in var_nodes {
                        palace.add_edge(tmpl_idx, var_idx, EdgeKind::References);
                    }
                }
            }
        }

        Ok(())
    }

    fn analyze_playbooks(&self, root: &Path, palace: &mut Palace) -> Result<()> {
        let playbooks_dir = root.join("playbooks");
        if !playbooks_dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&playbooks_dir)?.filter_map(std::result::Result::ok) {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "yml" && ext != "yaml" {
                continue;
            }

            let content = std::fs::read_to_string(&path)?;
            let plays: Vec<serde_yaml::Value> = serde_yaml::from_str(&content).unwrap_or_default();

            for play in &plays {
                let play_name = play
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unnamed play");

                let play_node = Node::new(NodeKind::Task, play_name, &path, Span::new(1, 1, 0, 0))
                    .with_visibility(Visibility::Public)
                    .with_signature(format!("play: {play_name}"));
                let play_idx = palace.add_node(play_node);

                // Link to roles
                if let Some(roles) = play.get("roles")
                    && let Some(roles_seq) = roles.as_sequence()
                {
                    for role_val in roles_seq {
                        let role_name = role_val
                            .as_str()
                            .or_else(|| role_val.get("role").and_then(|v| v.as_str()));
                        if let Some(name) = role_name {
                            let role_nodes = palace.find_by_name(name).to_vec();
                            for role_idx in role_nodes {
                                palace.add_edge(play_idx, role_idx, EdgeKind::DependsOn);
                            }
                        }
                    }
                }

                // Extract variable references from tasks in the play
                if let Some(tasks) = play.get("tasks")
                    && let Some(tasks_seq) = tasks.as_sequence()
                {
                    for (i, task) in tasks_seq.iter().enumerate() {
                        if let Some(name) = task.get("name").and_then(|v| v.as_str()) {
                            let task_node = Node::new(
                                NodeKind::Task,
                                name,
                                &path,
                                Span::new(i as u32 + 1, i as u32 + 1, 0, 0),
                            );
                            let task_idx = palace.add_node(task_node);
                            palace.add_edge(play_idx, task_idx, EdgeKind::Contains);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn analyze_group_vars(&self, root: &Path, palace: &mut Palace) -> Result<()> {
        let group_vars_dir = root.join("group_vars");
        if !group_vars_dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&group_vars_dir)?.filter_map(std::result::Result::ok) {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "yml" && ext != "yaml" {
                continue;
            }

            let content = std::fs::read_to_string(&path)?;
            let vars: serde_yaml::Value =
                serde_yaml::from_str(&content).unwrap_or(serde_yaml::Value::Null);

            if let serde_yaml::Value::Mapping(map) = vars {
                for (i, (key, _)) in map.iter().enumerate() {
                    if let Some(var_name) = key.as_str() {
                        let var_node = Node::new(
                            NodeKind::Variable,
                            var_name,
                            &path,
                            Span::new(i as u32 + 1, i as u32 + 1, 0, 0),
                        )
                        .with_visibility(Visibility::Public);
                        palace.add_node(var_node);
                    }
                }
            }
        }

        Ok(())
    }
}

impl Analyzer for AnsibleAnalyzer {
    fn can_handle(&self, facet: &ProjectFacet) -> bool {
        matches!(facet, ProjectFacet::Ansible)
    }

    fn analyze(&self, root: &Path, palace: &mut Palace) -> Result<()> {
        // Order matters: variables first so tasks can reference them
        self.analyze_group_vars(root, palace)?;
        self.analyze_roles(root, palace)?;
        self.analyze_playbooks(root, palace)?;
        Ok(())
    }

    fn analyze_file(&self, _path: &Path, _source: &str, _palace: &mut Palace) -> Result<()> {
        // Ansible needs full project context, single file analysis is limited
        Ok(())
    }
}

// ========================
// Terraform Analyzer
// ========================

pub struct TerraformAnalyzer {
    // Simple regex-based parser for HCL
    block_re: Regex,
    ref_re: Regex,
}

impl TerraformAnalyzer {
    /// # Panics
    /// Panics if the built-in Terraform block regex is invalid (should never happen).
    #[must_use]
    pub fn new() -> Self {
        Self {
            // Match: resource "type" "name" {, variable "name" {, etc.
            block_re: Regex::new(
                r#"(?m)^(resource|data|variable|output|module|locals)\s+"([^"]+)"(?:\s+"([^"]+)")?\s*\{"#,
            )
            .unwrap(),
            // Match: var.name, module.name.output, data.type.name
            ref_re: Regex::new(r"\b(var|module|data|local)\.([\w.]+)").unwrap(),
        }
    }

    fn walk_tf_files(&self, root: &Path) -> Vec<PathBuf> {
        ignore::WalkBuilder::new(root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
            .map(ignore::DirEntry::into_path)
            .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("tf"))
            .collect()
    }

    fn parse_tf_file(&self, path: &Path, palace: &mut Palace) -> Result<()> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let file_node = Node::new(
            NodeKind::File,
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"),
            path,
            Span::new(1, content.lines().count() as u32, 0, 0),
        );
        let file_idx = palace.add_node(file_node);

        // Extract blocks
        for cap in self.block_re.captures_iter(&content) {
            let block_type = &cap[1];
            let type_or_name = &cap[2];
            let name = cap.get(3).map(|m| m.as_str());

            let (node_kind, display_name, sig) = match block_type {
                "resource" => {
                    let n = name.unwrap_or(type_or_name);
                    (
                        NodeKind::Resource,
                        n.to_string(),
                        Some(format!("resource \"{type_or_name}\" \"{n}\"")),
                    )
                }
                "data" => {
                    let n = name.unwrap_or(type_or_name);
                    (
                        NodeKind::Resource,
                        format!("data.{n}"),
                        Some(format!("data \"{type_or_name}\" \"{n}\"")),
                    )
                }
                "variable" => (
                    NodeKind::Variable,
                    type_or_name.to_string(),
                    Some(format!("variable \"{type_or_name}\"")),
                ),
                "output" => (
                    NodeKind::Variable,
                    format!("output.{type_or_name}"),
                    Some(format!("output \"{type_or_name}\"")),
                ),
                "module" => (
                    NodeKind::Module,
                    type_or_name.to_string(),
                    Some(format!("module \"{type_or_name}\"")),
                ),
                _ => (NodeKind::Variable, type_or_name.to_string(), None),
            };

            let byte_offset = cap.get(0).unwrap().start();
            let line = content[..byte_offset].lines().count() as u32 + 1;

            let mut node = Node::new(node_kind, display_name, path, Span::new(line, line, 0, 0))
                .with_visibility(Visibility::Public);
            if let Some(s) = sig {
                node = node.with_signature(s);
            }
            let node_idx = palace.add_node(node);
            palace.add_edge(file_idx, node_idx, EdgeKind::Contains);
        }

        // Extract references (var.x, module.x, data.x)
        for cap in self.ref_re.captures_iter(&content) {
            let ref_type = &cap[1];
            let ref_name = &cap[2];
            let short_name = ref_name.split('.').next().unwrap_or(ref_name);

            let target_name = match ref_type {
                "data" => format!("data.{short_name}"),
                _ => short_name.to_string(),
            };

            let targets = palace.find_by_name(&target_name).to_vec();
            for target_idx in targets {
                palace.add_edge(file_idx, target_idx, EdgeKind::References);
            }
        }

        Ok(())
    }
}

impl Analyzer for TerraformAnalyzer {
    fn can_handle(&self, facet: &ProjectFacet) -> bool {
        matches!(facet, ProjectFacet::Terraform)
    }

    fn analyze(&self, root: &Path, palace: &mut Palace) -> Result<()> {
        let files = self.walk_tf_files(root);
        for file in files {
            self.parse_tf_file(&file, palace)?;
        }
        Ok(())
    }

    fn analyze_file(&self, path: &Path, _source: &str, palace: &mut Palace) -> Result<()> {
        self.parse_tf_file(path, palace)
    }
}
