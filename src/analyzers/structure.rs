use anyhow::Result;
use async_trait::async_trait;

use crate::analyzers::traits::{Analyzer, AnalyzerCategory, Issue, Severity};
use crate::core::project::Project;
use crate::frameworks::detector::Framework;
use crate::utils::fs;

pub struct StructureAnalyzer;

impl StructureAnalyzer {
    fn required_dirs(framework: &Framework) -> Vec<&'static str> {
        match framework {
            Framework::Symfony => vec!["src", "config", "templates"],
            Framework::Laravel => vec!["app", "config", "resources", "routes"],
            Framework::Flutter => vec!["lib", "test"],
            Framework::NextJs => vec!["pages", "public"],
            Framework::RustCargo => vec!["src"],
            Framework::NodeJs => vec!["src"],
            Framework::Python => vec!["src"],
            Framework::Unknown => vec![],
        }
    }

    fn forbidden_paths() -> Vec<&'static str> {
        vec!["node_modules", ".env", "dist/credentials"]
    }
}

#[async_trait]
impl Analyzer for StructureAnalyzer {
    fn name(&self) -> &'static str {
        "structure"
    }

    fn description(&self) -> &'static str {
        "Analyzes project directory structure and essential files"
    }

    fn category(&self) -> AnalyzerCategory {
        AnalyzerCategory::Structure
    }

    fn applies_to(&self, _project: &Project) -> bool {
        true
    }

    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let path = &project.path;

        // STR-001: Check required directories
        let required = Self::required_dirs(&project.detected.framework);
        for dir in required {
            if !fs::path_exists(path, dir) {
                issues.push(Issue {
                    id: "STR-001".to_string(),
                    analyzer: self.name().to_string(),
                    category: AnalyzerCategory::Structure,
                    severity: Severity::High,
                    title: format!("Missing required directory: {}", dir),
                    description: format!(
                        "The '{}' directory is expected for {} projects.",
                        dir, project.detected.framework
                    ),
                    file: None,
                    line: None,
                    suggestion: Some(format!("Create the '{}' directory", dir)),
                    auto_fixable: true,
                    references: vec![],
                });
            }
        }

        // STR-002: Check README.md
        if !fs::path_exists(path, "README.md") {
            issues.push(Issue {
                id: "STR-002".to_string(),
                analyzer: self.name().to_string(),
                category: AnalyzerCategory::Structure,
                severity: Severity::Medium,
                title: "Missing README.md".to_string(),
                description: "A README.md file is essential for project documentation.".to_string(),
                file: None,
                line: None,
                suggestion: Some("Create a README.md with project description and usage instructions".to_string()),
                auto_fixable: false,
                references: vec![],
            });
        }

        // STR-003: Check .gitignore
        if !fs::path_exists(path, ".gitignore") {
            issues.push(Issue {
                id: "STR-003".to_string(),
                analyzer: self.name().to_string(),
                category: AnalyzerCategory::Structure,
                severity: Severity::High,
                title: "Missing .gitignore".to_string(),
                description: "A .gitignore file prevents committing unwanted files.".to_string(),
                file: None,
                line: None,
                suggestion: Some("Create a .gitignore appropriate for your framework".to_string()),
                auto_fixable: true,
                references: vec![],
            });
        }

        // STR-004: Check LICENSE
        if !fs::path_exists(path, "LICENSE") && !fs::path_exists(path, "LICENSE.md") {
            issues.push(Issue {
                id: "STR-004".to_string(),
                analyzer: self.name().to_string(),
                category: AnalyzerCategory::Structure,
                severity: Severity::Low,
                title: "Missing LICENSE file".to_string(),
                description: "A LICENSE file clarifies how others can use your code.".to_string(),
                file: None,
                line: None,
                suggestion: Some("Add a LICENSE file (MIT, Apache-2.0, etc.)".to_string()),
                auto_fixable: false,
                references: vec![],
            });
        }

        // STR-005: Check max directory depth
        let max_depth = fs::max_directory_depth(path);
        if max_depth > 8 {
            issues.push(Issue {
                id: "STR-005".to_string(),
                analyzer: self.name().to_string(),
                category: AnalyzerCategory::Structure,
                severity: Severity::Medium,
                title: format!("Excessive directory depth: {}", max_depth),
                description: "Deep nesting makes code harder to navigate and maintain.".to_string(),
                file: None,
                line: None,
                suggestion: Some("Consider flattening your directory structure (max recommended: 8 levels)".to_string()),
                auto_fixable: false,
                references: vec![],
            });
        }

        // STR-006: Check forbidden paths
        for forbidden in Self::forbidden_paths() {
            if fs::path_exists(path, forbidden) {
                issues.push(Issue {
                    id: "STR-006".to_string(),
                    analyzer: self.name().to_string(),
                    category: AnalyzerCategory::Structure,
                    severity: Severity::Critical,
                    title: format!("Forbidden path found: {}", forbidden),
                    description: format!(
                        "The path '{}' should not be in the repository.",
                        forbidden
                    ),
                    file: Some(path.join(forbidden)),
                    line: None,
                    suggestion: Some(format!("Remove '{}' and add it to .gitignore", forbidden)),
                    auto_fixable: false,
                    references: vec![],
                });
            }
        }

        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::detector::{DetectedProject, Framework, Language};
    use std::fs as stdfs;
    use tempfile::TempDir;

    fn make_project(tmp: &TempDir, framework: Framework) -> Project {
        let language = match &framework {
            Framework::RustCargo => Language::Rust,
            Framework::NodeJs | Framework::NextJs => Language::JavaScript,
            _ => Language::Unknown,
        };
        Project {
            path: tmp.path().to_path_buf(),
            detected: DetectedProject {
                framework,
                language,
                version: None,
                package_manager: None,
                has_git: false,
                has_ci: None,
            },
        }
    }

    #[tokio::test]
    async fn test_missing_readme() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let analyzer = StructureAnalyzer;
        let issues = analyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "STR-002"));
    }

    #[tokio::test]
    async fn test_has_readme_no_issue() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join("README.md"), "# Hello").unwrap();
        stdfs::write(tmp.path().join(".gitignore"), "/target").unwrap();
        stdfs::write(tmp.path().join("LICENSE"), "MIT").unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let analyzer = StructureAnalyzer;
        let issues = analyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "STR-002"));
        assert!(!issues.iter().any(|i| i.id == "STR-003"));
        assert!(!issues.iter().any(|i| i.id == "STR-004"));
    }

    #[tokio::test]
    async fn test_missing_gitignore() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let analyzer = StructureAnalyzer;
        let issues = analyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "STR-003"));
    }

    #[tokio::test]
    async fn test_missing_license() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let analyzer = StructureAnalyzer;
        let issues = analyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "STR-004"));
    }

    #[tokio::test]
    async fn test_rust_missing_src_dir() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let analyzer = StructureAnalyzer;
        let issues = analyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "STR-001" && i.title.contains("src")));
    }

    #[tokio::test]
    async fn test_rust_has_src_dir() {
        let tmp = TempDir::new().unwrap();
        stdfs::create_dir(tmp.path().join("src")).unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let analyzer = StructureAnalyzer;
        let issues = analyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "STR-001"));
    }

    #[tokio::test]
    async fn test_forbidden_path_node_modules() {
        let tmp = TempDir::new().unwrap();
        stdfs::create_dir(tmp.path().join("node_modules")).unwrap();
        let project = make_project(&tmp, Framework::NodeJs);
        let analyzer = StructureAnalyzer;
        let issues = analyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "STR-006" && i.severity == Severity::Critical));
    }

    #[tokio::test]
    async fn test_excessive_depth() {
        let tmp = TempDir::new().unwrap();
        stdfs::create_dir_all(tmp.path().join("a/b/c/d/e/f/g/h/i/j")).unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let analyzer = StructureAnalyzer;
        let issues = analyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "STR-005"));
    }

    #[tokio::test]
    async fn test_applies_to_all() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let analyzer = StructureAnalyzer;
        assert!(analyzer.applies_to(&project));
    }
}
