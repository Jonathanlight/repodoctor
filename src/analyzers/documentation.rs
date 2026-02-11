use anyhow::Result;
use async_trait::async_trait;

use crate::analyzers::traits::{Analyzer, AnalyzerCategory, Issue, Severity};
use crate::core::project::Project;

pub struct DocumentationAnalyzer;

#[async_trait]
impl Analyzer for DocumentationAnalyzer {
    fn name(&self) -> &'static str {
        "documentation"
    }

    fn description(&self) -> &'static str {
        "Checks documentation quality and completeness"
    }

    fn category(&self) -> AnalyzerCategory {
        AnalyzerCategory::Documentation
    }

    fn applies_to(&self, _project: &Project) -> bool {
        true
    }

    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let path = &project.path;

        // DOC-001: Check README exists and has minimum content
        let readme_path = path.join("README.md");
        if readme_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&readme_path) {
                let lines: Vec<&str> = content.lines().collect();
                if lines.len() < 5 {
                    issues.push(Issue {
                        id: "DOC-001".to_string(),
                        analyzer: "documentation".to_string(),
                        category: AnalyzerCategory::Documentation,
                        severity: Severity::Medium,
                        title: "README.md is too short".to_string(),
                        description: "A good README should have at least a description, installation instructions, and usage examples.".to_string(),
                        file: Some("README.md".into()),
                        line: None,
                        suggestion: Some("Add sections: Description, Installation, Usage".to_string()),
                        auto_fixable: false,
                        references: vec![],
                    });
                } else {
                    let lower = content.to_lowercase();
                    let required_sections = [
                        ("DOC-002", "install", "Installation"),
                        ("DOC-006", "usage", "Usage"),
                    ];
                    for (rule_id, keyword, section_name) in &required_sections {
                        if !lower.contains(keyword) {
                            issues.push(Issue {
                                id: rule_id.to_string(),
                                analyzer: "documentation".to_string(),
                                category: AnalyzerCategory::Documentation,
                                severity: Severity::Low,
                                title: format!("README.md missing {} section", section_name),
                                description: format!("Consider adding a {} section to help users get started.", section_name),
                                file: Some("README.md".into()),
                                line: None,
                                suggestion: Some(format!("Add a ## {} section", section_name)),
                                auto_fixable: false,
                                references: vec![],
                            });
                        }
                    }
                }
            }
        }

        // DOC-003: Check CONTRIBUTING.md exists
        if !path.join("CONTRIBUTING.md").exists() {
            issues.push(Issue {
                id: "DOC-003".to_string(),
                analyzer: "documentation".to_string(),
                category: AnalyzerCategory::Documentation,
                severity: Severity::Info,
                title: "Missing CONTRIBUTING.md".to_string(),
                description: "A CONTRIBUTING.md helps new contributors understand how to participate.".to_string(),
                file: None,
                line: None,
                suggestion: Some("Create a CONTRIBUTING.md with guidelines for contributors".to_string()),
                auto_fixable: false,
                references: vec![],
            });
        }

        // DOC-004: Check LICENSE file has content
        let license_path = path.join("LICENSE");
        let license_md_path = path.join("LICENSE.md");
        let license_file = if license_path.exists() {
            Some(license_path)
        } else if license_md_path.exists() {
            Some(license_md_path)
        } else {
            None
        };

        if let Some(lf) = license_file {
            if let Ok(content) = std::fs::read_to_string(&lf) {
                if content.trim().len() < 50 {
                    issues.push(Issue {
                        id: "DOC-004".to_string(),
                        analyzer: "documentation".to_string(),
                        category: AnalyzerCategory::Documentation,
                        severity: Severity::Medium,
                        title: "LICENSE file appears incomplete".to_string(),
                        description: "The LICENSE file exists but has very little content.".to_string(),
                        file: Some(lf.file_name().unwrap().into()),
                        line: None,
                        suggestion: Some("Add a proper license text (MIT, Apache 2.0, etc.)".to_string()),
                        auto_fixable: false,
                        references: vec!["https://choosealicense.com".to_string()],
                    });
                }
            }
        }

        // DOC-005: Check for code of conduct
        if !path.join("CODE_OF_CONDUCT.md").exists() {
            issues.push(Issue {
                id: "DOC-005".to_string(),
                analyzer: "documentation".to_string(),
                category: AnalyzerCategory::Documentation,
                severity: Severity::Info,
                title: "Missing CODE_OF_CONDUCT.md".to_string(),
                description: "A code of conduct sets expectations for community behavior.".to_string(),
                file: None,
                line: None,
                suggestion: Some("Add a CODE_OF_CONDUCT.md (e.g., Contributor Covenant)".to_string()),
                auto_fixable: false,
                references: vec!["https://www.contributor-covenant.org".to_string()],
            });
        }

        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::detector::{DetectedProject, Framework, Language};
    use std::fs;
    use tempfile::TempDir;

    fn make_project(tmp: &TempDir) -> Project {
        Project {
            path: tmp.path().to_path_buf(),
            detected: DetectedProject {
                framework: Framework::RustCargo,
                language: Language::Rust,
                version: None,
                package_manager: None,
                has_git: false,
                has_ci: None,
            },
        }
    }

    #[tokio::test]
    async fn test_applies_to_all() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);
        assert!(DocumentationAnalyzer.applies_to(&project));
    }

    #[tokio::test]
    async fn test_missing_contributing() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);
        let issues = DocumentationAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "DOC-003"));
    }

    #[tokio::test]
    async fn test_has_contributing() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("CONTRIBUTING.md"), "# Contributing\nGuidelines here.").unwrap();
        let project = make_project(&tmp);
        let issues = DocumentationAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "DOC-003"));
    }

    #[tokio::test]
    async fn test_short_readme() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("README.md"), "# Title\n").unwrap();
        let project = make_project(&tmp);
        let issues = DocumentationAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "DOC-001"));
    }

    #[tokio::test]
    async fn test_good_readme() {
        let tmp = TempDir::new().unwrap();
        let content = "# My Project\n\nDescription here.\n\n## Installation\n\nRun install.\n\n## Usage\n\nUse it.\n";
        fs::write(tmp.path().join("README.md"), content).unwrap();
        let project = make_project(&tmp);
        let issues = DocumentationAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "DOC-001"));
        assert!(!issues.iter().any(|i| i.id == "DOC-002"));
    }

    #[tokio::test]
    async fn test_readme_missing_usage_section() {
        let tmp = TempDir::new().unwrap();
        let content = "# My Project\n\nDescription here.\n\n## Installation\n\nRun install.\n\nMore details.\n";
        fs::write(tmp.path().join("README.md"), content).unwrap();
        let project = make_project(&tmp);
        let issues = DocumentationAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "DOC-001")); // not too short
        assert!(issues.iter().any(|i| i.id == "DOC-006" && i.title.contains("Usage")));
    }

    #[tokio::test]
    async fn test_incomplete_license() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("LICENSE"), "MIT").unwrap();
        let project = make_project(&tmp);
        let issues = DocumentationAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "DOC-004"));
    }

    #[tokio::test]
    async fn test_valid_license() {
        let tmp = TempDir::new().unwrap();
        let license_text = "MIT License\n\nCopyright (c) 2024\n\nPermission is hereby granted, free of charge, to any person obtaining a copy of this software...";
        fs::write(tmp.path().join("LICENSE"), license_text).unwrap();
        let project = make_project(&tmp);
        let issues = DocumentationAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "DOC-004"));
    }

    #[tokio::test]
    async fn test_missing_code_of_conduct() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);
        let issues = DocumentationAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "DOC-005"));
    }

    #[tokio::test]
    async fn test_has_code_of_conduct() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("CODE_OF_CONDUCT.md"), "# Code of Conduct\n").unwrap();
        let project = make_project(&tmp);
        let issues = DocumentationAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "DOC-005"));
    }
}
