use std::path::Path;

/// A facet of a project — a project can have multiple facets (e.g., Rust + Ansible)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProjectFacet {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Go,
    C,
    Cpp,
    CSharp,
    Ansible,
    Terraform,
    Docker,
    Markdown,
    Unknown,
}

impl ProjectFacet {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Go => "go",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Ansible => "ansible",
            Self::Terraform => "terraform",
            Self::Docker => "docker",
            Self::Markdown => "markdown",
            Self::Unknown => "unknown",
        }
    }
}

/// Detect project facets by scanning for marker files in the root directory
pub fn detect(root: &Path) -> Vec<ProjectFacet> {
    let mut facets = Vec::new();

    if root.join("Cargo.toml").exists() {
        facets.push(ProjectFacet::Rust);
    }
    if root.join("pyproject.toml").exists()
        || root.join("setup.py").exists()
        || root.join("requirements.txt").exists()
    {
        facets.push(ProjectFacet::Python);
    }
    if root.join("tsconfig.json").exists() {
        facets.push(ProjectFacet::TypeScript);
    } else if root.join("package.json").exists() {
        facets.push(ProjectFacet::JavaScript);
    }
    if root.join("go.mod").exists() {
        facets.push(ProjectFacet::Go);
    }
    if root.join("CMakeLists.txt").exists()
        || root.join("Makefile").exists()
        || root.join("meson.build").exists()
        || has_extension_in_root(root, "c")
        || has_extension_in_root(root, "h")
    {
        facets.push(ProjectFacet::C);
    }
    if has_extension_in_root(root, "cpp")
        || has_extension_in_root(root, "cc")
        || has_extension_in_root(root, "cxx")
        || has_extension_in_root(root, "hpp")
    {
        facets.push(ProjectFacet::Cpp);
    }
    if has_extension_in_root(root, "sln")
        || has_extension_in_root(root, "csproj")
        || has_extension_in_root(root, "cs")
        || has_extension_in_subdir(root, "csproj")
        || has_extension_in_subdir(root, "cs")
        || has_extension_in_subdir(&root.join("src"), "csproj")
        || has_extension_in_subdir(&root.join("src"), "cs")
    {
        facets.push(ProjectFacet::CSharp);
    }
    if root.join("ansible.cfg").exists()
        || root.join("playbooks").is_dir()
        || root.join("roles").is_dir()
    {
        facets.push(ProjectFacet::Ansible);
    }
    if has_extension_in_root(root, "tf") {
        facets.push(ProjectFacet::Terraform);
    }
    if root.join("Dockerfile").exists() || root.join("docker-compose.yml").exists() {
        facets.push(ProjectFacet::Docker);
    }

    // Markdown as fallback if mostly .md files
    if facets.is_empty() && is_mostly_markdown(root) {
        facets.push(ProjectFacet::Markdown);
    }

    if facets.is_empty() {
        facets.push(ProjectFacet::Unknown);
    }

    facets
}

fn has_extension_in_root(root: &Path, ext: &str) -> bool {
    root.read_dir()
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().map(|e| e == ext).unwrap_or(false))
        })
        .unwrap_or(false)
}

/// Check immediate subdirectories (one level) for files with given extension.
/// Only scans non-hidden, non-build dirs to stay fast on large repos.
fn has_extension_in_subdir(root: &Path, ext: &str) -> bool {
    const SKIP: &[&str] = &[
        "target",
        "node_modules",
        ".git",
        "__pycache__",
        "vendor",
        "dist",
        "build",
        "bin",
        "obj",
    ];
    root.read_dir()
        .ok()
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| {
                let p = e.path();
                if !p.is_dir() {
                    return false;
                }
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || SKIP.contains(&name) {
                    return false;
                }
                has_extension_in_root(&p, ext)
            })
        })
        .unwrap_or(false)
}

fn is_mostly_markdown(root: &Path) -> bool {
    let entries: Vec<_> = root
        .read_dir()
        .ok()
        .map(|e| e.filter_map(|e| e.ok()).collect())
        .unwrap_or_default();

    if entries.is_empty() {
        return false;
    }

    let md_count = entries
        .iter()
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "md" || ext == "mdx" || ext == "rst")
                .unwrap_or(false)
        })
        .count();

    let file_count = entries.iter().filter(|e| e.path().is_file()).count();
    file_count > 0 && md_count * 2 >= file_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_detect_rust() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        let facets = detect(dir.path());
        assert!(facets.contains(&ProjectFacet::Rust));
    }

    #[test]
    fn test_detect_empty_is_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let facets = detect(dir.path());
        assert_eq!(facets, vec![ProjectFacet::Unknown]);
    }

    #[test]
    fn test_detect_mixed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        fs::create_dir(dir.path().join("roles")).unwrap();
        let facets = detect(dir.path());
        assert!(facets.contains(&ProjectFacet::Rust));
        assert!(facets.contains(&ProjectFacet::Ansible));
    }
}
