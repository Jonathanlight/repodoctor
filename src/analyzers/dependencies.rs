use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

use crate::analyzers::traits::{Analyzer, AnalyzerCategory, Issue, Severity};
use crate::core::project::Project;
use crate::frameworks::detector::{Framework, PackageManager};
use crate::utils::fs::path_exists;

pub struct DependenciesAnalyzer;

#[async_trait]
impl Analyzer for DependenciesAnalyzer {
    fn name(&self) -> &'static str {
        "dependencies"
    }

    fn description(&self) -> &'static str {
        "Checks dependency management, lock files, and dependency hygiene"
    }

    fn category(&self) -> AnalyzerCategory {
        AnalyzerCategory::Dependencies
    }

    fn applies_to(&self, project: &Project) -> bool {
        project.detected.package_manager.is_some()
    }

    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let path = &project.path;

        match project.detected.framework {
            Framework::RustCargo => check_rust(path, &mut issues),
            Framework::NodeJs | Framework::NextJs => check_node(path, &mut issues),
            Framework::Symfony | Framework::Laravel => check_php(path, &mut issues),
            Framework::Flutter => check_flutter(path, &mut issues),
            Framework::Python => check_python(path, &mut issues),
            Framework::Unknown => {}
        }

        Ok(issues)
    }
}

fn check_rust(path: &Path, issues: &mut Vec<Issue>) {
    // Check lock file
    if !path_exists(path, "Cargo.lock") {
        issues.push(Issue {
            id: "DEP-001".to_string(),
            analyzer: "dependencies".to_string(),
            category: AnalyzerCategory::Dependencies,
            severity: Severity::High,
            title: "Missing Cargo.lock".to_string(),
            description: "No Cargo.lock found. Lock files ensure reproducible builds."
                .to_string(),
            file: None,
            line: None,
            suggestion: Some("Run `cargo build` to generate Cargo.lock".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }

    // Parse Cargo.toml for dependencies
    let cargo_path = path.join("Cargo.toml");
    if let Ok(content) = std::fs::read_to_string(&cargo_path) {
        let dep_count = count_cargo_dependencies(&content);
        if dep_count == 0 {
            issues.push(Issue {
                id: "DEP-002".to_string(),
                analyzer: "dependencies".to_string(),
                category: AnalyzerCategory::Dependencies,
                severity: Severity::Info,
                title: "No dependencies declared".to_string(),
                description: "Cargo.toml has no [dependencies] entries.".to_string(),
                file: Some(cargo_path.clone()),
                line: None,
                suggestion: None,
                auto_fixable: false,
                references: vec![],
            });
        } else if dep_count > 50 {
            issues.push(Issue {
                id: "DEP-005".to_string(),
                analyzer: "dependencies".to_string(),
                category: AnalyzerCategory::Dependencies,
                severity: Severity::Low,
                title: format!("Too many direct dependencies ({dep_count})"),
                description: format!(
                    "Project has {dep_count} direct dependencies. Consider reducing to improve compile times."
                ),
                file: Some(cargo_path),
                line: None,
                suggestion: Some(
                    "Review dependencies and remove unused ones".to_string(),
                ),
                auto_fixable: false,
                references: vec![],
            });
        }
    }
}

fn count_cargo_dependencies(content: &str) -> usize {
    let mut in_deps = false;
    let mut count = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if in_deps && !trimmed.is_empty() && !trimmed.starts_with('#') && trimmed.contains('=') {
            count += 1;
        }
    }
    count
}

fn check_node(path: &Path, issues: &mut Vec<Issue>) {
    // Check lock file
    let has_lock = path_exists(path, "package-lock.json")
        || path_exists(path, "yarn.lock")
        || path_exists(path, "pnpm-lock.yaml");

    if !has_lock {
        issues.push(Issue {
            id: "DEP-001".to_string(),
            analyzer: "dependencies".to_string(),
            category: AnalyzerCategory::Dependencies,
            severity: Severity::High,
            title: "Missing lock file".to_string(),
            description: "No package-lock.json, yarn.lock, or pnpm-lock.yaml found.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Run `npm install` to generate a lock file".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }

    // Parse package.json
    let pkg_path = path.join("package.json");
    if let Ok(content) = std::fs::read_to_string(&pkg_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            let deps = json
                .get("dependencies")
                .and_then(|v| v.as_object())
                .map(|o| o.len())
                .unwrap_or(0);

            let dev_deps = json
                .get("devDependencies")
                .and_then(|v| v.as_object());

            if deps == 0 && dev_deps.map(|d| d.len()).unwrap_or(0) == 0 {
                issues.push(Issue {
                    id: "DEP-002".to_string(),
                    analyzer: "dependencies".to_string(),
                    category: AnalyzerCategory::Dependencies,
                    severity: Severity::Info,
                    title: "No dependencies declared".to_string(),
                    description: "package.json has no dependencies or devDependencies."
                        .to_string(),
                    file: Some(pkg_path.clone()),
                    line: None,
                    suggestion: None,
                    auto_fixable: false,
                    references: vec![],
                });
            }

            // Check for dev dependencies in production section
            if let Some(prod_deps) = json.get("dependencies").and_then(|v| v.as_object()) {
                let dev_in_prod: Vec<&str> = prod_deps
                    .keys()
                    .filter(|k| is_node_dev_dependency(k))
                    .map(|k| k.as_str())
                    .collect();

                if !dev_in_prod.is_empty() {
                    issues.push(Issue {
                        id: "DEP-003".to_string(),
                        analyzer: "dependencies".to_string(),
                        category: AnalyzerCategory::Dependencies,
                        severity: Severity::Medium,
                        title: "Dev dependencies in production section".to_string(),
                        description: format!(
                            "These packages are likely devDependencies but are listed in dependencies: {}",
                            dev_in_prod.join(", ")
                        ),
                        file: Some(pkg_path.clone()),
                        line: None,
                        suggestion: Some(
                            "Move development-only packages to devDependencies".to_string(),
                        ),
                        auto_fixable: false,
                        references: vec![],
                    });
                }
            }

            if deps > 50 {
                issues.push(Issue {
                    id: "DEP-005".to_string(),
                    analyzer: "dependencies".to_string(),
                    category: AnalyzerCategory::Dependencies,
                    severity: Severity::Low,
                    title: format!("Too many direct dependencies ({deps})"),
                    description: format!(
                        "package.json has {deps} production dependencies. Consider reducing bundle size."
                    ),
                    file: Some(pkg_path),
                    line: None,
                    suggestion: Some(
                        "Review dependencies and remove unused ones".to_string(),
                    ),
                    auto_fixable: false,
                    references: vec![],
                });
            }
        }
    }
}

fn is_node_dev_dependency(name: &str) -> bool {
    let dev_prefixes = [
        "eslint",
        "@types/",
        "prettier",
        "jest",
        "mocha",
        "chai",
        "typescript",
        "ts-node",
        "nodemon",
        "webpack",
        "babel",
        "@babel/",
        "rollup",
        "vite",
    ];
    let lower = name.to_lowercase();
    dev_prefixes.iter().any(|p| lower.starts_with(p))
}

fn check_php(path: &Path, issues: &mut Vec<Issue>) {
    if !path_exists(path, "composer.lock") {
        issues.push(Issue {
            id: "DEP-001".to_string(),
            analyzer: "dependencies".to_string(),
            category: AnalyzerCategory::Dependencies,
            severity: Severity::High,
            title: "Missing composer.lock".to_string(),
            description: "No composer.lock found. Lock files ensure reproducible builds."
                .to_string(),
            file: None,
            line: None,
            suggestion: Some("Run `composer install` to generate composer.lock".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }

    let composer_path = path.join("composer.json");
    if let Ok(content) = std::fs::read_to_string(&composer_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            let deps = json
                .get("require")
                .and_then(|v| v.as_object())
                .map(|o| o.len())
                .unwrap_or(0);
            let dev_deps = json
                .get("require-dev")
                .and_then(|v| v.as_object())
                .map(|o| o.len())
                .unwrap_or(0);

            if deps == 0 && dev_deps == 0 {
                issues.push(Issue {
                    id: "DEP-002".to_string(),
                    analyzer: "dependencies".to_string(),
                    category: AnalyzerCategory::Dependencies,
                    severity: Severity::Info,
                    title: "No dependencies declared".to_string(),
                    description: "composer.json has no require or require-dev entries."
                        .to_string(),
                    file: Some(composer_path.clone()),
                    line: None,
                    suggestion: None,
                    auto_fixable: false,
                    references: vec![],
                });
            }

            // Check dev deps in production
            if let Some(prod_deps) = json.get("require").and_then(|v| v.as_object()) {
                let dev_in_prod: Vec<&str> = prod_deps
                    .keys()
                    .filter(|k| is_php_dev_dependency(k))
                    .map(|k| k.as_str())
                    .collect();

                if !dev_in_prod.is_empty() {
                    issues.push(Issue {
                        id: "DEP-003".to_string(),
                        analyzer: "dependencies".to_string(),
                        category: AnalyzerCategory::Dependencies,
                        severity: Severity::Medium,
                        title: "Dev dependencies in production section".to_string(),
                        description: format!(
                            "These packages are likely require-dev but are in require: {}",
                            dev_in_prod.join(", ")
                        ),
                        file: Some(composer_path.clone()),
                        line: None,
                        suggestion: Some(
                            "Move development-only packages to require-dev".to_string(),
                        ),
                        auto_fixable: false,
                        references: vec![],
                    });
                }
            }

            if deps > 50 {
                issues.push(Issue {
                    id: "DEP-005".to_string(),
                    analyzer: "dependencies".to_string(),
                    category: AnalyzerCategory::Dependencies,
                    severity: Severity::Low,
                    title: format!("Too many direct dependencies ({deps})"),
                    description: format!(
                        "composer.json has {deps} production dependencies."
                    ),
                    file: Some(composer_path),
                    line: None,
                    suggestion: Some(
                        "Review dependencies and remove unused ones".to_string(),
                    ),
                    auto_fixable: false,
                    references: vec![],
                });
            }
        }
    }
}

fn is_php_dev_dependency(name: &str) -> bool {
    let dev_packages = [
        "phpunit/",
        "phpstan/",
        "squizlabs/",
        "friendsofphp/",
        "vimeo/psalm",
        "mockery/",
        "fakerphp/",
    ];
    let lower = name.to_lowercase();
    dev_packages.iter().any(|p| lower.starts_with(p))
}

fn check_flutter(path: &Path, issues: &mut Vec<Issue>) {
    if !path_exists(path, "pubspec.lock") {
        issues.push(Issue {
            id: "DEP-001".to_string(),
            analyzer: "dependencies".to_string(),
            category: AnalyzerCategory::Dependencies,
            severity: Severity::High,
            title: "Missing pubspec.lock".to_string(),
            description: "No pubspec.lock found. Lock files ensure reproducible builds."
                .to_string(),
            file: None,
            line: None,
            suggestion: Some("Run `flutter pub get` to generate pubspec.lock".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_python(path: &Path, issues: &mut Vec<Issue>) {
    let has_requirements = path_exists(path, "requirements.txt");
    let has_pyproject = path_exists(path, "pyproject.toml");

    if !has_requirements && !has_pyproject {
        issues.push(Issue {
            id: "DEP-002".to_string(),
            analyzer: "dependencies".to_string(),
            category: AnalyzerCategory::Dependencies,
            severity: Severity::Info,
            title: "No dependencies declared".to_string(),
            description: "No requirements.txt or pyproject.toml found.".to_string(),
            file: None,
            line: None,
            suggestion: None,
            auto_fixable: false,
            references: vec![],
        });
    }

    // Check for unpinned versions in requirements.txt
    if has_requirements {
        let req_path = path.join("requirements.txt");
        if let Ok(content) = std::fs::read_to_string(&req_path) {
            let unpinned: Vec<String> = content
                .lines()
                .filter(|l| {
                    let trimmed = l.trim();
                    !trimmed.is_empty()
                        && !trimmed.starts_with('#')
                        && !trimmed.starts_with('-')
                        && !trimmed.contains("==")
                })
                .map(|l| l.trim().to_string())
                .collect();

            if !unpinned.is_empty() {
                issues.push(Issue {
                    id: "DEP-004".to_string(),
                    analyzer: "dependencies".to_string(),
                    category: AnalyzerCategory::Dependencies,
                    severity: Severity::Medium,
                    title: "Unpinned dependency versions".to_string(),
                    description: format!(
                        "These dependencies lack pinned versions (==): {}",
                        unpinned.join(", ")
                    ),
                    file: Some(req_path),
                    line: None,
                    suggestion: Some(
                        "Pin versions with == for reproducible builds (e.g., requests==2.28.0)"
                            .to_string(),
                    ),
                    auto_fixable: false,
                    references: vec![],
                });
            }
        }
    }

    // Check lock file for pip-based projects
    if has_requirements
        && !path_exists(path, "requirements.lock")
        && !path_exists(path, "poetry.lock")
    {
        // Python pip projects often don't have lock files, but we note it
        // Only flag if using poetry (which should have poetry.lock)
        if let Some(PackageManager::Poetry) = &project_pm(path) {
            if !path_exists(path, "poetry.lock") {
                issues.push(Issue {
                    id: "DEP-001".to_string(),
                    analyzer: "dependencies".to_string(),
                    category: AnalyzerCategory::Dependencies,
                    severity: Severity::High,
                    title: "Missing poetry.lock".to_string(),
                    description: "No poetry.lock found. Lock files ensure reproducible builds."
                        .to_string(),
                    file: None,
                    line: None,
                    suggestion: Some(
                        "Run `poetry lock` to generate poetry.lock".to_string(),
                    ),
                    auto_fixable: false,
                    references: vec![],
                });
            }
        }
    }
}

fn project_pm(path: &Path) -> Option<PackageManager> {
    if path.join("pyproject.toml").exists() {
        if let Ok(content) = std::fs::read_to_string(path.join("pyproject.toml")) {
            if content.contains("[tool.poetry]") {
                return Some(PackageManager::Poetry);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::detector::{DetectedProject, Language};
    use std::fs as stdfs;
    use tempfile::TempDir;

    fn make_project(tmp: &TempDir, framework: Framework, pm: Option<PackageManager>) -> Project {
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
                package_manager: pm,
                has_git: false,
                has_ci: None,
            },
        }
    }

    #[tokio::test]
    async fn test_rust_missing_lock_file() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(
            tmp.path().join("Cargo.toml"),
            "[dependencies]\nserde = \"1\"",
        )
        .unwrap();
        let project = make_project(&tmp, Framework::RustCargo, Some(PackageManager::Cargo));
        let issues = DependenciesAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "DEP-001"));
    }

    #[tokio::test]
    async fn test_rust_has_lock_file() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(
            tmp.path().join("Cargo.toml"),
            "[dependencies]\nserde = \"1\"",
        )
        .unwrap();
        stdfs::write(tmp.path().join("Cargo.lock"), "# lock").unwrap();
        let project = make_project(&tmp, Framework::RustCargo, Some(PackageManager::Cargo));
        let issues = DependenciesAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "DEP-001"));
    }

    #[tokio::test]
    async fn test_rust_no_dependencies() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"foo\"").unwrap();
        stdfs::write(tmp.path().join("Cargo.lock"), "# lock").unwrap();
        let project = make_project(&tmp, Framework::RustCargo, Some(PackageManager::Cargo));
        let issues = DependenciesAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "DEP-002"));
    }

    #[tokio::test]
    async fn test_node_missing_lock_file() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(
            tmp.path().join("package.json"),
            r#"{"dependencies":{"express":"^4.0"}}"#,
        )
        .unwrap();
        let project = make_project(&tmp, Framework::NodeJs, Some(PackageManager::Npm));
        let issues = DependenciesAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "DEP-001"));
    }

    #[tokio::test]
    async fn test_node_dev_deps_in_production() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(
            tmp.path().join("package.json"),
            r#"{"dependencies":{"eslint":"^8.0","express":"^4.0"}}"#,
        )
        .unwrap();
        stdfs::write(tmp.path().join("package-lock.json"), "{}").unwrap();
        let project = make_project(&tmp, Framework::NodeJs, Some(PackageManager::Npm));
        let issues = DependenciesAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "DEP-003"));
    }

    #[tokio::test]
    async fn test_python_unpinned_versions() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(
            tmp.path().join("requirements.txt"),
            "requests>=2.28\nflask\ndjango==4.2",
        )
        .unwrap();
        let project = make_project(&tmp, Framework::Python, Some(PackageManager::Pip));
        let issues = DependenciesAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "DEP-004"));
    }

    #[tokio::test]
    async fn test_python_all_pinned() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(
            tmp.path().join("requirements.txt"),
            "requests==2.28.0\nflask==2.3.0",
        )
        .unwrap();
        let project = make_project(&tmp, Framework::Python, Some(PackageManager::Pip));
        let issues = DependenciesAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "DEP-004"));
    }

    #[tokio::test]
    async fn test_applies_only_with_package_manager() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Unknown, None);
        assert!(!DependenciesAnalyzer.applies_to(&project));

        let project2 = make_project(&tmp, Framework::RustCargo, Some(PackageManager::Cargo));
        assert!(DependenciesAnalyzer.applies_to(&project2));
    }

    #[tokio::test]
    async fn test_flutter_missing_lock() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Flutter, Some(PackageManager::Pub));
        let issues = DependenciesAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "DEP-001"));
    }
}
