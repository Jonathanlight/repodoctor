use anyhow::Result;
use async_trait::async_trait;

use crate::analyzers::traits::{Analyzer, AnalyzerCategory, Issue, Severity};
use crate::core::project::Project;
use crate::frameworks::detector::Framework;
use crate::utils::fs;

pub struct TestingAnalyzer;

impl TestingAnalyzer {
    fn test_dirs(framework: &Framework) -> Vec<&'static str> {
        match framework {
            Framework::Symfony | Framework::Laravel => vec!["tests"],
            Framework::Flutter => vec!["test"],
            Framework::NextJs | Framework::NodeJs => {
                vec!["__tests__", "tests", "test", "spec"]
            }
            Framework::RustCargo => vec!["tests"],
            Framework::Python => vec!["tests", "test"],
            Framework::Unknown => vec!["tests", "test", "__tests__", "spec"],
        }
    }

    fn test_configs(framework: &Framework) -> Vec<&'static str> {
        match framework {
            Framework::Symfony | Framework::Laravel => vec!["phpunit.xml", "phpunit.xml.dist"],
            Framework::Flutter => vec!["test"],
            Framework::NextJs | Framework::NodeJs => {
                vec![
                    "jest.config.js",
                    "jest.config.ts",
                    "vitest.config.js",
                    "vitest.config.ts",
                    ".mocharc.yml",
                    ".mocharc.json",
                ]
            }
            Framework::RustCargo => vec![],
            Framework::Python => vec![
                "pytest.ini",
                "pyproject.toml",
                "setup.cfg",
                "tox.ini",
            ],
            Framework::Unknown => vec![],
        }
    }

    fn count_source_files(path: &std::path::Path, framework: &Framework) -> usize {
        let extensions: Vec<&str> = match framework {
            Framework::Symfony | Framework::Laravel => vec!["php"],
            Framework::Flutter => vec!["dart"],
            Framework::NextJs | Framework::NodeJs => vec!["js", "ts", "jsx", "tsx"],
            Framework::RustCargo => vec!["rs"],
            Framework::Python => vec!["py"],
            Framework::Unknown => vec!["rs", "py", "js", "ts", "php", "dart"],
        };

        let src_dirs = ["src", "lib", "app"];
        let mut count = 0;

        for dir in &src_dirs {
            let dir_path = path.join(dir);
            if dir_path.is_dir() {
                for entry in walkdir::WalkDir::new(&dir_path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        if let Some(ext) = entry.path().extension() {
                            if extensions.contains(&ext.to_string_lossy().as_ref()) {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        count
    }

    fn count_test_files(path: &std::path::Path, framework: &Framework) -> usize {
        let extensions: Vec<&str> = match framework {
            Framework::Symfony | Framework::Laravel => vec!["php"],
            Framework::Flutter => vec!["dart"],
            Framework::NextJs | Framework::NodeJs => vec!["js", "ts", "jsx", "tsx"],
            Framework::RustCargo => vec!["rs"],
            Framework::Python => vec!["py"],
            Framework::Unknown => vec!["rs", "py", "js", "ts", "php", "dart"],
        };

        let test_dirs = Self::test_dirs(framework);
        let mut count = 0;

        for dir in &test_dirs {
            let dir_path = path.join(dir);
            if dir_path.is_dir() {
                for entry in walkdir::WalkDir::new(&dir_path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        if let Some(ext) = entry.path().extension() {
                            if extensions.contains(&ext.to_string_lossy().as_ref()) {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        count
    }
}

#[async_trait]
impl Analyzer for TestingAnalyzer {
    fn name(&self) -> &'static str {
        "testing"
    }

    fn description(&self) -> &'static str {
        "Checks testing setup, configuration, and coverage"
    }

    fn category(&self) -> AnalyzerCategory {
        AnalyzerCategory::Testing
    }

    fn applies_to(&self, _project: &Project) -> bool {
        true
    }

    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let path = &project.path;
        let framework = &project.detected.framework;

        // TST-001: Check for test directory
        let test_dirs = Self::test_dirs(framework);
        let has_test_dir = test_dirs.iter().any(|d| fs::path_exists(path, d));

        if !has_test_dir {
            issues.push(Issue {
                id: "TST-001".to_string(),
                analyzer: "testing".to_string(),
                category: AnalyzerCategory::Testing,
                severity: Severity::High,
                title: "No test directory found".to_string(),
                description: format!(
                    "Expected one of: {}",
                    test_dirs.join(", ")
                ),
                file: None,
                line: None,
                suggestion: Some(format!("Create a {} directory with test files", test_dirs[0])),
                auto_fixable: false,
                references: vec![],
            });
        }

        // TST-002: Check for test configuration
        let test_configs = Self::test_configs(framework);
        if !test_configs.is_empty() {
            let has_test_config = test_configs.iter().any(|c| fs::path_exists(path, c));
            if !has_test_config {
                issues.push(Issue {
                    id: "TST-002".to_string(),
                    analyzer: "testing".to_string(),
                    category: AnalyzerCategory::Testing,
                    severity: Severity::Medium,
                    title: "No test configuration found".to_string(),
                    description: format!(
                        "Expected one of: {}",
                        test_configs.join(", ")
                    ),
                    file: None,
                    line: None,
                    suggestion: Some("Add a test configuration file for your testing framework".to_string()),
                    auto_fixable: false,
                    references: vec![],
                });
            }
        }

        // TST-003: Check test-to-source ratio
        let source_count = Self::count_source_files(path, framework);
        let test_count = Self::count_test_files(path, framework);

        if source_count > 0 && has_test_dir {
            if test_count == 0 {
                issues.push(Issue {
                    id: "TST-003".to_string(),
                    analyzer: "testing".to_string(),
                    category: AnalyzerCategory::Testing,
                    severity: Severity::High,
                    title: "Test directory exists but contains no test files".to_string(),
                    description: format!(
                        "Found {} source files but 0 test files.",
                        source_count
                    ),
                    file: None,
                    line: None,
                    suggestion: Some("Add test files to cover your source code".to_string()),
                    auto_fixable: false,
                    references: vec![],
                });
            } else {
                let ratio = test_count as f64 / source_count as f64;
                if ratio < 0.2 {
                    issues.push(Issue {
                        id: "TST-004".to_string(),
                        analyzer: "testing".to_string(),
                        category: AnalyzerCategory::Testing,
                        severity: Severity::Medium,
                        title: "Low test-to-source file ratio".to_string(),
                        description: format!(
                            "Found {} test files for {} source files (ratio: {:.0}%). Consider adding more tests.",
                            test_count, source_count, ratio * 100.0
                        ),
                        file: None,
                        line: None,
                        suggestion: Some("Aim for at least 1 test file per 3 source files".to_string()),
                        auto_fixable: false,
                        references: vec![],
                    });
                }
            }
        }

        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::detector::{DetectedProject, Language};
    use std::fs;
    use tempfile::TempDir;

    fn make_project(tmp: &TempDir, framework: Framework) -> Project {
        let (language, pm) = match framework {
            Framework::RustCargo => (Language::Rust, None),
            Framework::NodeJs => (Language::JavaScript, None),
            Framework::Symfony => (Language::Php, None),
            Framework::Flutter => (Language::Dart, None),
            _ => (Language::Unknown, None),
        };
        Project {
            path: tmp.path().to_path_buf(),
            detected: DetectedProject {
                framework,
                language,
                version: None,
                package_manager: pm,
                has_git: false,
                has_ci: None,
            },
        }
    }

    #[tokio::test]
    async fn test_applies_to_all() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        assert!(TestingAnalyzer.applies_to(&project));
    }

    #[tokio::test]
    async fn test_no_test_dir() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let issues = TestingAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "TST-001"));
    }

    #[tokio::test]
    async fn test_has_test_dir() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("tests")).unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let issues = TestingAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "TST-001"));
    }

    #[tokio::test]
    async fn test_node_missing_test_config() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::NodeJs);
        let issues = TestingAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "TST-002"));
    }

    #[tokio::test]
    async fn test_node_has_jest_config() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("jest.config.js"), "module.exports = {}").unwrap();
        let project = make_project(&tmp, Framework::NodeJs);
        let issues = TestingAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "TST-002"));
    }

    #[tokio::test]
    async fn test_rust_no_test_config_check() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let issues = TestingAnalyzer.analyze(&project).await.unwrap();
        // Rust uses cargo test natively, no config needed
        assert!(!issues.iter().any(|i| i.id == "TST-002"));
    }

    #[tokio::test]
    async fn test_empty_test_dir() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();
        fs::create_dir(tmp.path().join("tests")).unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let issues = TestingAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "TST-003"));
    }

    #[tokio::test]
    async fn test_low_test_ratio() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("src")).unwrap();
        for i in 0..10 {
            fs::write(tmp.path().join(format!("src/mod{}.rs", i)), "// src").unwrap();
        }
        fs::create_dir(tmp.path().join("tests")).unwrap();
        fs::write(tmp.path().join("tests/test1.rs"), "// test").unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let issues = TestingAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "TST-004"));
    }

    #[tokio::test]
    async fn test_good_test_ratio() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("src")).unwrap();
        for i in 0..3 {
            fs::write(tmp.path().join(format!("src/mod{}.rs", i)), "// src").unwrap();
        }
        fs::create_dir(tmp.path().join("tests")).unwrap();
        for i in 0..3 {
            fs::write(tmp.path().join(format!("tests/test{}.rs", i)), "// test").unwrap();
        }
        let project = make_project(&tmp, Framework::RustCargo);
        let issues = TestingAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "TST-004"));
    }

    #[tokio::test]
    async fn test_flutter_test_dir() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("test")).unwrap();
        let project = make_project(&tmp, Framework::Flutter);
        let issues = TestingAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "TST-001"));
    }
}
