use anyhow::Result;
use arbor_core::graph::{EdgeKind, Node, NodeKind, Span, Visibility};
use arbor_core::palace::Palace;
use arbor_detect::ProjectFacet;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::path::{Path, PathBuf};

use crate::Analyzer;

pub struct DocsAnalyzer;

impl DocsAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn walk_md_files(&self, root: &Path) -> Vec<PathBuf> {
        ignore::WalkBuilder::new(root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|entry| entry.into_path())
            .filter(|path| {
                path.extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| ext == "md" || ext == "mdx" || ext == "rst")
                    .unwrap_or(false)
            })
            .collect()
    }

    fn parse_markdown(&self, path: &Path, palace: &mut Palace) -> Result<()> {
        let content = std::fs::read_to_string(path)?;
        let total_lines = content.lines().count() as u32;

        // Document node
        let doc_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let doc_node = Node::new(
            NodeKind::Document,
            doc_name,
            path,
            Span::new(1, total_lines, 0, 0),
        )
        .with_visibility(Visibility::Public);
        let doc_idx = palace.add_node(doc_node);

        let parser = Parser::new(&content);
        let mut current_heading: Option<String> = None;
        let mut in_heading = false;
        let mut heading_text = String::new();
        let mut line_counter = 1u32;

        // Track heading hierarchy for parent-child
        let mut heading_stack: Vec<(petgraph::stable_graph::NodeIndex, u8)> = Vec::new();

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    in_heading = true;
                    heading_text.clear();
                    let level_num = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    current_heading = Some(format!("h{}", level_num));
                }
                Event::Text(text) if in_heading => {
                    heading_text.push_str(&text);
                }
                Event::End(TagEnd::Heading(_)) => {
                    in_heading = false;
                    if let Some(ref level_str) = current_heading {
                        let level: u8 = level_str[1..].parse().unwrap_or(1);
                        let section_node = Node::new(
                            NodeKind::Section,
                            &heading_text,
                            path,
                            Span::new(line_counter, line_counter, 0, 0),
                        )
                        .with_signature(format!("{} {}", "#".repeat(level as usize), heading_text));
                        let section_idx = palace.add_node(section_node);

                        // Pop stack until we find a parent with lower level
                        while heading_stack
                            .last()
                            .map(|(_, l)| *l >= level)
                            .unwrap_or(false)
                        {
                            heading_stack.pop();
                        }

                        let parent = heading_stack
                            .last()
                            .map(|(idx, _)| *idx)
                            .unwrap_or(doc_idx);
                        palace.add_edge(parent, section_idx, EdgeKind::Contains);
                        heading_stack.push((section_idx, level));
                    }
                    current_heading = None;
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    let url = dest_url.to_string();
                    // Check if it's a relative link to another file
                    if !url.starts_with("http") && !url.starts_with('#') {
                        let link_path = path.parent().unwrap_or(path).join(&url);
                        // Try to find the target document
                        let target_name = link_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("");
                        let targets = palace.find_by_name(target_name).to_vec();
                        for target_idx in targets {
                            palace.add_edge(doc_idx, target_idx, EdgeKind::LinksTo);
                        }
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    line_counter += 1;
                }
                _ => {}
            }
        }

        Ok(())
    }
}

impl Analyzer for DocsAnalyzer {
    fn can_handle(&self, facet: &ProjectFacet) -> bool {
        matches!(facet, ProjectFacet::Markdown)
    }

    fn analyze(&self, root: &Path, palace: &mut Palace) -> Result<()> {
        let files = self.walk_md_files(root);
        for file in files {
            self.parse_markdown(&file, palace)?;
        }
        Ok(())
    }

    fn analyze_file(&self, path: &Path, _source: &str, palace: &mut Palace) -> Result<()> {
        self.parse_markdown(path, palace)
    }
}
