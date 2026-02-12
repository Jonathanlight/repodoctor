use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

use crate::analyzers::traits::{Analyzer, AnalyzerCategory, Issue, Severity};
use crate::core::project::Project;
use crate::frameworks::detector::Framework;
use crate::utils::fs::path_exists;

pub struct ConfigAnalyzer;

#[async_trait]
impl Analyzer for ConfigAnalyzer {
    fn name(&self) -> &'static str {
        "config_files"
    }

    fn description(&self) -> &'static str {
        "Checks for framework-specific configuration files and common config issues"
    }

    fn category(&self) -> AnalyzerCategory {
        AnalyzerCategory::Configuration
    }

    fn applies_to(&self, _project: &Project) -> bool {
        true
    }

    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let path = &project.path;

        // Framework-specific config checks
        check_framework_config(path, &project.detected.framework, &mut issues);

        // Linter checks
        check_linter_config(path, &project.detected.framework, &mut issues);

        // Generic checks
        check_editorconfig(path, &mut issues);
        check_env_committed(path, &mut issues);

        Ok(issues)
    }
}

fn check_framework_config(path: &Path, framework: &Framework, issues: &mut Vec<Issue>) {
    let missing_configs: Vec<(&str, &str)> = match framework {
        Framework::Symfony => {
            let mut missing = Vec::new();
            if !path_exists(path, ".env.example") && !path_exists(path, ".env.dist") {
                missing.push((".env.example", "Environment example file for team onboarding"));
            }
            if !path_exists(path, "config/packages/doctrine.yaml") {
                missing.push((
                    "config/packages/doctrine.yaml",
                    "Doctrine ORM configuration",
                ));
            }
            if !path_exists(path, "config/packages/security.yaml") {
                missing.push((
                    "config/packages/security.yaml",
                    "Security configuration",
                ));
            }
            missing
        }
        Framework::Laravel => {
            let mut missing = Vec::new();
            if !path_exists(path, ".env.example") {
                missing.push((".env.example", "Environment example file for team onboarding"));
            }
            if !path_exists(path, "config/app.php") {
                missing.push(("config/app.php", "Application configuration"));
            }
            if !path_exists(path, "config/database.php") {
                missing.push(("config/database.php", "Database configuration"));
            }
            missing
        }
        Framework::Flutter => {
            let mut missing = Vec::new();
            if !path_exists(path, "analysis_options.yaml") {
                missing.push(("analysis_options.yaml", "Dart analysis options for linting"));
            }
            missing
        }
        Framework::NextJs => {
            let mut missing = Vec::new();
            if !path_exists(path, "tsconfig.json") && !path_exists(path, "jsconfig.json") {
                missing.push((
                    "tsconfig.json",
                    "TypeScript/JavaScript configuration for path aliases and compiler options",
                ));
            }
            missing
        }
        Framework::RustCargo => {
            let mut missing = Vec::new();
            if !path_exists(path, "rustfmt.toml") && !path_exists(path, ".rustfmt.toml") {
                missing.push(("rustfmt.toml", "Rust formatter configuration"));
            }
            missing
        }
        Framework::Python => {
            let mut missing = Vec::new();
            if !path_exists(path, "setup.cfg") && !has_pyproject_tool_section(path) {
                missing.push(("setup.cfg or pyproject.toml [tool.*]", "Python tooling configuration"));
            }
            missing
        }
        Framework::NodeJs | Framework::Unknown => Vec::new(),
    };

    for (file, desc) in missing_configs {
        issues.push(Issue {
            id: "CFG-001".to_string(),
            analyzer: "config_files".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Medium,
            title: format!("Missing {file}"),
            description: format!("{desc}. This file is recommended for {} projects.", framework),
            file: None,
            line: None,
            suggestion: Some(format!("Create {file}")),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn has_pyproject_tool_section(path: &Path) -> bool {
    if let Ok(content) = std::fs::read_to_string(path.join("pyproject.toml")) {
        content.contains("[tool.")
    } else {
        false
    }
}

fn check_linter_config(path: &Path, framework: &Framework, issues: &mut Vec<Issue>) {
    let has_linter = match framework {
        Framework::Flutter => path_exists(path, "analysis_options.yaml"),
        Framework::RustCargo => {
            path_exists(path, "clippy.toml") || path_exists(path, ".clippy.toml")
        }
        Framework::NodeJs | Framework::NextJs => {
            has_eslint_config(path) || has_prettier_config(path)
        }
        Framework::Python => {
            path_exists(path, ".flake8")
                || path_exists(path, "setup.cfg")
                || path_exists(path, ".pylintrc")
                || has_pyproject_tool_section(path)
        }
        Framework::Symfony | Framework::Laravel => {
            path_exists(path, "phpstan.neon")
                || path_exists(path, "phpstan.neon.dist")
                || path_exists(path, ".php-cs-fixer.php")
                || path_exists(path, ".php-cs-fixer.dist.php")
        }
        Framework::Unknown => return,
    };

    if !has_linter {
        issues.push(Issue {
            id: "CFG-004".to_string(),
            analyzer: "config_files".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Medium,
            title: "Missing linter configuration".to_string(),
            description: format!(
                "No linter or code style configuration found for {} project.",
                framework
            ),
            file: None,
            line: None,
            suggestion: Some("Add a linter configuration file to enforce code quality".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn has_eslint_config(path: &Path) -> bool {
    path_exists(path, ".eslintrc")
        || path_exists(path, ".eslintrc.js")
        || path_exists(path, ".eslintrc.cjs")
        || path_exists(path, ".eslintrc.json")
        || path_exists(path, ".eslintrc.yml")
        || path_exists(path, ".eslintrc.yaml")
        || path_exists(path, "eslint.config.js")
        || path_exists(path, "eslint.config.mjs")
        || path_exists(path, "eslint.config.cjs")
}

fn has_prettier_config(path: &Path) -> bool {
    path_exists(path, ".prettierrc")
        || path_exists(path, ".prettierrc.js")
        || path_exists(path, ".prettierrc.json")
        || path_exists(path, ".prettierrc.yml")
        || path_exists(path, ".prettierrc.yaml")
        || path_exists(path, "prettier.config.js")
}

fn check_editorconfig(path: &Path, issues: &mut Vec<Issue>) {
    if !path_exists(path, ".editorconfig") {
        issues.push(Issue {
            id: "CFG-002".to_string(),
            analyzer: "config_files".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Low,
            title: "Missing .editorconfig".to_string(),
            description: "No .editorconfig found. This file helps maintain consistent coding styles across editors.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create an .editorconfig file to define coding style rules".to_string()),
            auto_fixable: true,
            references: vec!["https://editorconfig.org".to_string()],
        });
    }
}

fn check_env_committed(path: &Path, issues: &mut Vec<Issue>) {
    if !path_exists(path, ".env") {
        return;
    }

    // Check if .env is gitignored
    let gitignore_path = path.join(".gitignore");
    let is_gitignored = if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
        content
            .lines()
            .any(|line| {
                let trimmed = line.trim();
                trimmed == ".env" || trimmed == "/.env" || trimmed == ".env*"
            })
    } else {
        false
    };

    if !is_gitignored {
        issues.push(Issue {
            id: "CFG-003".to_string(),
            analyzer: "config_files".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Critical,
            title: ".env file found in project root".to_string(),
            description:
                ".env file exists and may not be gitignored. This could lead to secret leaks."
                    .to_string(),
            file: Some(path.join(".env")),
            line: None,
            suggestion: Some("Add .env to .gitignore to prevent committing secrets".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::detector::{DetectedProject, Language};
    use std::fs as stdfs;
    use tempfile::TempDir;

    fn make_project(tmp: &TempDir, framework: Framework) -> Project {
        let language = match &framework {
            Framework::RustCargo => Language::Rust,
            Framework::NodeJs | Framework::NextJs => Language::JavaScript,
            Framework::Symfony | Framework::Laravel => Language::Php,
            Framework::Flutter => Language::Dart,
            Framework::Python => Language::Python,
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
    async fn test_missing_editorconfig() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let issues = ConfigAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "CFG-002"));
    }

    #[tokio::test]
    async fn test_has_editorconfig() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join(".editorconfig"), "root = true").unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let issues = ConfigAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "CFG-002"));
    }

    #[tokio::test]
    async fn test_env_committed_not_gitignored() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join(".env"), "SECRET=foo").unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let issues = ConfigAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "CFG-003" && i.severity == Severity::Critical));
    }

    #[tokio::test]
    async fn test_env_gitignored() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join(".env"), "SECRET=foo").unwrap();
        stdfs::write(tmp.path().join(".gitignore"), ".env\n").unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let issues = ConfigAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "CFG-003"));
    }

    #[tokio::test]
    async fn test_rust_missing_rustfmt() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let issues = ConfigAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "CFG-001" && i.title.contains("rustfmt")));
    }

    #[tokio::test]
    async fn test_rust_has_rustfmt() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join("rustfmt.toml"), "max_width = 100").unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let issues = ConfigAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "CFG-001" && i.title.contains("rustfmt")));
    }

    #[tokio::test]
    async fn test_node_missing_linter() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::NodeJs);
        let issues = ConfigAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "CFG-004"));
    }

    #[tokio::test]
    async fn test_node_has_eslint() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join(".eslintrc.json"), "{}").unwrap();
        let project = make_project(&tmp, Framework::NodeJs);
        let issues = ConfigAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "CFG-004"));
    }

    #[tokio::test]
    async fn test_applies_to_all() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        assert!(ConfigAnalyzer.applies_to(&project));
    }
}
