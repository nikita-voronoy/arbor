#![allow(clippy::new_without_default)]

pub mod code;
pub mod docs;
pub mod iac;
pub mod schema;

use anyhow::Result;
use arbor_core::palace::Palace;
use arbor_detect::ProjectFacet;
use std::path::Path;

/// Trait that all analyzers implement
pub trait Analyzer {
    /// Which project facets this analyzer can handle
    fn can_handle(&self, facet: &ProjectFacet) -> bool;

    /// Analyze the entire project from root.
    ///
    /// # Errors
    /// Returns an error if file reading or parsing fails.
    fn analyze(&self, root: &Path, palace: &mut Palace) -> Result<()>;

    /// Analyze a single file (for incremental updates).
    ///
    /// # Errors
    /// Returns an error if parsing fails.
    fn analyze_file(&self, path: &Path, source: &str, palace: &mut Palace) -> Result<()>;
}

/// Registry of all available analyzers
pub struct AnalyzerRegistry {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl AnalyzerRegistry {
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self {
            analyzers: Vec::new(),
        };
        registry.analyzers.push(Box::new(code::CodeAnalyzer::new()));
        registry
            .analyzers
            .push(Box::new(iac::AnsibleAnalyzer::new()));
        registry
            .analyzers
            .push(Box::new(iac::TerraformAnalyzer::new()));
        registry.analyzers.push(Box::new(docs::DocsAnalyzer::new()));
        registry
            .analyzers
            .push(Box::new(schema::SchemaAnalyzer::new()));
        registry
    }

    /// Get analyzers that can handle a given facet
    pub fn for_facet(&self, facet: &ProjectFacet) -> Vec<&dyn Analyzer> {
        self.analyzers
            .iter()
            .filter(|a| a.can_handle(facet))
            .map(std::convert::AsRef::as_ref)
            .collect()
    }

    /// Get all analyzers that can handle any of the given facets (deduplicated)
    pub fn for_facets(&self, facets: &[ProjectFacet]) -> Vec<&dyn Analyzer> {
        self.analyzers
            .iter()
            .filter(|a| facets.iter().any(|f| a.can_handle(f)))
            .map(std::convert::AsRef::as_ref)
            .collect()
    }

    /// Analyze a project: detect facets, run matching analyzers, resolve cross-file calls.
    ///
    /// # Errors
    /// Returns an error if any analyzer fails.
    pub fn analyze_project(&self, root: &Path, palace: &mut Palace) -> Result<Vec<ProjectFacet>> {
        let facets = arbor_detect::detect(root);
        for facet in &facets {
            for analyzer in self.for_facet(facet) {
                analyzer.analyze(root, palace)?;
            }
        }
        palace.resolve_pending_calls();
        Ok(facets)
    }
}

impl Default for AnalyzerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
