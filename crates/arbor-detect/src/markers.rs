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
    Kotlin,
    Java,
    Ansible,
    Terraform,
    Docker,
    Markdown,
    Unknown,
}

impl ProjectFacet {
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Go => "go",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Kotlin => "kotlin",
            Self::Java => "java",
            Self::Ansible => "ansible",
            Self::Terraform => "terraform",
            Self::Docker => "docker",
            Self::Markdown => "markdown",
            Self::Unknown => "unknown",
        }
    }
}

/// Detect project facets by scanning for marker files in the root directory
#[must_use]
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
    if root.join("tsconfig.json").exists()
        || has_marker_in_subdir(root, "tsconfig.json")
        || has_extension_in_subdir(root, "ts")
        || has_extension_in_subdir(root, "tsx")
    {
        facets.push(ProjectFacet::TypeScript);
    } else if root.join("package.json").exists()
        || has_marker_in_subdir(root, "package.json")
        || has_extension_in_subdir(root, "js")
        || has_extension_in_subdir(root, "jsx")
    {
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
    if root.join("build.gradle.kts").exists()
        || root.join("build.gradle").exists()
        || has_extension_in_root(root, "kt")
        || has_extension_in_root(root, "kts")
        || has_extension_in_subdir(root, "kt")
        || has_extension_in_subdir(&root.join("src"), "kt")
    {
        facets.push(ProjectFacet::Kotlin);
    }
    if root.join("pom.xml").exists()
        || has_extension_in_root(root, "java")
        || has_extension_in_subdir(root, "java")
        || has_extension_in_subdir(&root.join("src"), "java")
    {
        facets.push(ProjectFacet::Java);
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
        // Warn if code files exist in subdirectories — the user likely expected
        // a code language to be detected, not markdown.
        if has_code_files_in_subdirs(root) {
            eprintln!(
                "arbor: warning: detected project as markdown, but found code files in \
                 subdirectories. Consider adding a marker file (e.g. package.json, \
                 Cargo.toml) at the project root, or check that .gitignore excludes \
                 generated/vendored code."
            );
        }
        facets.push(ProjectFacet::Markdown);
    }

    if facets.is_empty() {
        facets.push(ProjectFacet::Unknown);
    }

    facets
}

fn has_extension_in_root(root: &Path, ext: &str) -> bool {
    root.read_dir().ok().is_some_and(|entries| {
        entries
            .filter_map(std::result::Result::ok)
            .any(|e| e.path().extension().is_some_and(|e| e == ext))
    })
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
    root.read_dir().ok().is_some_and(|entries| {
        entries.filter_map(std::result::Result::ok).any(|e| {
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
}

/// Check subdirectories (up to two levels) for a specific marker file (e.g. `package.json`).
/// This handles monorepo layouts like `packages/app/package.json` or `apps/web/tsconfig.json`.
fn has_marker_in_subdir(root: &Path, marker: &str) -> bool {
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
    let Ok(entries) = root.read_dir() else {
        return false;
    };
    for entry in entries.filter_map(std::result::Result::ok) {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || SKIP.contains(&name) {
            continue;
        }
        // Check level 1
        if p.join(marker).exists() {
            return true;
        }
        // Check level 2 (e.g. packages/app/package.json)
        if let Ok(sub_entries) = p.read_dir() {
            for sub_entry in sub_entries.filter_map(std::result::Result::ok) {
                let sp = sub_entry.path();
                if sp.is_dir() {
                    let sub_name = sp.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !sub_name.starts_with('.')
                        && !SKIP.contains(&sub_name)
                        && sp.join(marker).exists()
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check if code files (.js, .ts, .py, .rs, .go, etc.) exist in subdirectories.
/// Used to warn when falling back to markdown in a repo that contains actual code.
fn has_code_files_in_subdirs(root: &Path) -> bool {
    const CODE_EXTS: &[&str] = &[
        "js", "jsx", "ts", "tsx", "py", "rs", "go", "java", "kt", "cs", "c", "cpp", "cc", "cxx",
        "rb", "swift", "scala",
    ];
    for ext in CODE_EXTS {
        if has_extension_in_subdir(root, ext) {
            return true;
        }
    }
    false
}

fn is_mostly_markdown(root: &Path) -> bool {
    let entries: Vec<_> = root
        .read_dir()
        .ok()
        .map(|e| e.filter_map(std::result::Result::ok).collect())
        .unwrap_or_default();

    if entries.is_empty() {
        return false;
    }

    let md_count = entries
        .iter()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "md" || ext == "mdx" || ext == "rst")
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

    #[test]
    fn test_detect_js_monorepo_no_root_package_json() {
        let dir = tempfile::tempdir().unwrap();
        // Monorepo with package.json only in subdirectory
        let pkg_dir = dir.path().join("packages").join("app");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(pkg_dir.join("package.json"), r#"{"name":"app"}"#).unwrap();
        fs::write(pkg_dir.join("index.js"), "console.log('hi')").unwrap();
        let facets = detect(dir.path());
        assert!(
            facets.contains(&ProjectFacet::JavaScript),
            "expected JavaScript, got {facets:?}"
        );
    }

    #[test]
    fn test_detect_ts_monorepo_no_root_tsconfig() {
        let dir = tempfile::tempdir().unwrap();
        // Monorepo with tsconfig.json only in subdirectory
        let pkg_dir = dir.path().join("apps").join("web");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(pkg_dir.join("tsconfig.json"), "{}").unwrap();
        fs::write(pkg_dir.join("main.ts"), "export {}").unwrap();
        let facets = detect(dir.path());
        assert!(
            facets.contains(&ProjectFacet::TypeScript),
            "expected TypeScript, got {facets:?}"
        );
    }

    #[test]
    fn test_detect_js_files_in_subdir() {
        let dir = tempfile::tempdir().unwrap();
        // No marker files at all, just .js files in a subdirectory
        let src_dir = dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("app.js"), "module.exports = {}").unwrap();
        let facets = detect(dir.path());
        assert!(
            facets.contains(&ProjectFacet::JavaScript),
            "expected JavaScript, got {facets:?}"
        );
    }

    #[test]
    fn test_markdown_fallback_warns_with_code_in_subdirs() {
        let dir = tempfile::tempdir().unwrap();
        // Root has mostly markdown, but subdir has code
        // Since we now detect JS in subdirs, this should NOT fall back to markdown
        fs::write(dir.path().join("README.md"), "# Docs").unwrap();
        fs::write(dir.path().join("CONTRIBUTING.md"), "# Contributing").unwrap();
        let src_dir = dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("app.js"), "module.exports = {}").unwrap();
        let facets = detect(dir.path());
        // Should detect JavaScript, not Markdown
        assert!(
            facets.contains(&ProjectFacet::JavaScript),
            "expected JavaScript over Markdown fallback, got {facets:?}"
        );
    }

    #[test]
    fn test_skip_dist_and_build_in_subdir_detection() {
        let dir = tempfile::tempdir().unwrap();
        // Only code files in dist/ — should NOT detect as JavaScript
        let dist_dir = dir.path().join("dist");
        fs::create_dir_all(&dist_dir).unwrap();
        fs::write(dist_dir.join("bundle.js"), "var a=1").unwrap();
        let facets = detect(dir.path());
        assert!(
            !facets.contains(&ProjectFacet::JavaScript),
            "should not detect JS from dist/, got {facets:?}"
        );
    }
}
