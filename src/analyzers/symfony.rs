use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::analyzers::traits::{Analyzer, AnalyzerCategory, Issue, Severity};
use crate::core::project::Project;
use crate::frameworks::detector::Framework;

pub struct SymfonyAnalyzer;

/// Parsed subset of composer.json relevant to Symfony checks.
struct ComposerJson {
    require: HashMap<String, String>,
    require_dev: HashMap<String, String>,
}

impl ComposerJson {
    fn parse(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path.join("composer.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;

        let require = Self::parse_dep_map(json.get("require"));
        let require_dev = Self::parse_dep_map(json.get("require-dev"));

        Some(Self {
            require,
            require_dev,
        })
    }

    fn parse_dep_map(value: Option<&serde_json::Value>) -> HashMap<String, String> {
        value
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn has_require(&self, name: &str) -> bool {
        self.require.contains_key(name)
    }

    fn has_require_dev(&self, name: &str) -> bool {
        self.require_dev.contains_key(name)
    }
}

/// Directories to skip when walking the project tree.
const SKIP_DIRS: &[&str] = &["vendor", "var", ".git", "node_modules"];

#[async_trait]
impl Analyzer for SymfonyAnalyzer {
    fn name(&self) -> &'static str {
        "symfony"
    }

    fn description(&self) -> &'static str {
        "Symfony-specific project structure, configuration, and best practices"
    }

    fn category(&self) -> AnalyzerCategory {
        AnalyzerCategory::Structure
    }

    fn applies_to(&self, project: &Project) -> bool {
        project.detected.framework == Framework::Symfony
    }

    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let path = &project.path;
        let composer = ComposerJson::parse(path);

        // Structure checks
        check_missing_controller_dir(path, &mut issues);
        check_missing_entity_dir(path, &mut issues);
        check_misplaced_controllers(path, &mut issues);
        check_misplaced_services(path, &mut issues);

        // Configuration checks
        check_app_secret(path, &mut issues);
        check_prod_debug(path, &mut issues);

        // Dependencies checks
        if let Some(ref c) = composer {
            check_symfony_version(c, path, &mut issues);
            check_missing_runtime(c, path, &mut issues);
        }

        // Testing checks
        check_missing_phpunit_config(path, &mut issues);
        check_missing_tests_dir(path, &mut issues);
        if let Some(ref c) = composer {
            check_missing_phpunit_dep(c, path, &mut issues);
        }

        // Security checks
        check_hardcoded_db_credentials(path, &mut issues);
        if let Some(ref c) = composer {
            check_missing_cors_bundle(c, path, &mut issues);
        }
        check_unserialize_calls(path, &mut issues);

        // Best practices checks
        check_gitignore_entries(path, &mut issues);
        check_missing_rector(path, &mut issues);
        check_missing_phpstan(path, &mut issues);

        Ok(issues)
    }
}

// ---------------------------------------------------------------------------
// Structure checks
// ---------------------------------------------------------------------------

fn check_missing_controller_dir(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("src/Controller").is_dir() {
        issues.push(Issue {
            id: "SYM-001".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::High,
            title: "Missing src/Controller/ directory".to_string(),
            description: "Symfony projects should have a src/Controller/ directory for HTTP controllers.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create src/Controller/ and add your first controller".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_missing_entity_dir(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("src/Entity").is_dir() {
        issues.push(Issue {
            id: "SYM-002".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: "Missing src/Entity/ directory".to_string(),
            description: "Symfony projects typically use src/Entity/ for Doctrine entity classes.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create src/Entity/ if using Doctrine ORM".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_misplaced_controllers(path: &Path, issues: &mut Vec<Issue>) {
    for file in find_misplaced_php_files(path, "Controller.php", "src/Controller") {
        issues.push(Issue {
            id: "SYM-003".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: "Controller outside src/Controller/".to_string(),
            description: format!(
                "Controller file found outside the standard directory: {}",
                file.display()
            ),
            file: Some(file),
            line: None,
            suggestion: Some("Move controller files to src/Controller/".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_misplaced_services(path: &Path, issues: &mut Vec<Issue>) {
    for file in find_misplaced_php_files(path, "Service.php", "src/Service") {
        issues.push(Issue {
            id: "SYM-004".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Low,
            title: "Service outside src/Service/".to_string(),
            description: format!(
                "Service file found outside the standard directory: {}",
                file.display()
            ),
            file: Some(file),
            line: None,
            suggestion: Some("Move service files to src/Service/".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

/// Find PHP files ending with `suffix` that are NOT under `expected_dir`.
/// Skips vendor/, var/, .git/, node_modules/.
fn find_misplaced_php_files(base: &Path, suffix: &str, expected_dir: &str) -> Vec<PathBuf> {
    let expected = base.join(expected_dir);
    let mut results = Vec::new();

    for entry in WalkDir::new(base)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                return !SKIP_DIRS.iter().any(|d| name.as_ref() == *d);
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if !name.ends_with(suffix) {
            continue;
        }
        let file_path = entry.into_path();
        if !file_path.starts_with(&expected) {
            results.push(file_path);
        }
    }

    results
}

// ---------------------------------------------------------------------------
// Configuration checks
// ---------------------------------------------------------------------------

fn check_app_secret(path: &Path, issues: &mut Vec<Issue>) {
    let env_path = path.join(".env");
    let content = match std::fs::read_to_string(&env_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("APP_SECRET=") {
            continue;
        }
        let value = trimmed.trim_start_matches("APP_SECRET=").trim();
        let known_defaults = [
            "change_me",
            "your_app_secret",
            "ThisTokenIsNotSoSecretChangeIt",
            "somedefaultsecret",
        ];
        let is_weak = value.len() < 16
            || known_defaults
                .iter()
                .any(|d| value.eq_ignore_ascii_case(d));

        if is_weak {
            issues.push(Issue {
                id: "SYM-012".to_string(),
                analyzer: "symfony".to_string(),
                category: AnalyzerCategory::Configuration,
                severity: Severity::Critical,
                title: "Weak or default APP_SECRET".to_string(),
                description: "APP_SECRET in .env is a known default or shorter than 16 characters.".to_string(),
                file: Some(env_path.clone()),
                line: None,
                suggestion: Some("Generate a strong random secret: `php -r \"echo bin2hex(random_bytes(16));\"`".to_string()),
                auto_fixable: false,
                references: vec![],
            });
        }
        break;
    }
}

fn check_prod_debug(path: &Path, issues: &mut Vec<Issue>) {
    let prod_config = path.join("config/packages/prod");
    if !prod_config.is_dir() {
        return;
    }

    for entry in WalkDir::new(&prod_config)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if !name.ends_with(".yaml") && !name.ends_with(".yml") {
            continue;
        }
        let file_path = entry.path().to_path_buf();
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("debug:") && trimmed.contains("true") {
                    issues.push(Issue {
                        id: "SYM-013".to_string(),
                        analyzer: "symfony".to_string(),
                        category: AnalyzerCategory::Configuration,
                        severity: Severity::Critical,
                        title: "Debug enabled in production config".to_string(),
                        description: format!(
                            "debug: true found in production config file: {}",
                            file_path.display()
                        ),
                        file: Some(file_path.clone()),
                        line: None,
                        suggestion: Some("Remove or set debug: false in production configuration".to_string()),
                        auto_fixable: true,
                        references: vec![],
                    });
                    break;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Dependencies checks
// ---------------------------------------------------------------------------

/// Parse major version from a Symfony version constraint string.
/// Handles `^X.Y`, `~X.Y`, `>=X.Y`, `X.Y.*`, and bare `X.Y`.
fn parse_symfony_major_version(constraint: &str) -> Option<u32> {
    let cleaned = constraint
        .trim()
        .trim_start_matches('^')
        .trim_start_matches('~')
        .trim_start_matches(">=")
        .trim_start_matches("<=")
        .trim_start_matches('>')
        .trim_start_matches('<')
        .trim_start_matches('=')
        .trim();

    cleaned.split('.').next()?.parse::<u32>().ok()
}

fn check_symfony_version(composer: &ComposerJson, path: &Path, issues: &mut Vec<Issue>) {
    for (pkg, version) in &composer.require {
        if !pkg.starts_with("symfony/") {
            continue;
        }
        if let Some(major) = parse_symfony_major_version(version) {
            if major < 6 {
                issues.push(Issue {
                    id: "SYM-020".to_string(),
                    analyzer: "symfony".to_string(),
                    category: AnalyzerCategory::Dependencies,
                    severity: Severity::High,
                    title: format!("Outdated Symfony package: {} (v{})", pkg, major),
                    description: format!(
                        "{} requires version {} which is below Symfony 6. Consider upgrading.",
                        pkg, version
                    ),
                    file: Some(path.join("composer.json")),
                    line: None,
                    suggestion: Some("Upgrade to Symfony 6+ for long-term support and security fixes".to_string()),
                    auto_fixable: false,
                    references: vec![],
                });
                // Report once per project, not per package
                break;
            }
        }
    }
}

fn check_missing_runtime(composer: &ComposerJson, path: &Path, issues: &mut Vec<Issue>) {
    if !composer.has_require("symfony/runtime") {
        issues.push(Issue {
            id: "SYM-022".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Dependencies,
            severity: Severity::Low,
            title: "Missing symfony/runtime".to_string(),
            description: "symfony/runtime is not in require. It provides the Runtime component for better application bootstrapping.".to_string(),
            file: Some(path.join("composer.json")),
            line: None,
            suggestion: Some("Run `composer require symfony/runtime`".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Testing checks
// ---------------------------------------------------------------------------

fn check_missing_phpunit_config(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("phpunit.xml.dist").exists() && !path.join("phpunit.xml").exists() {
        issues.push(Issue {
            id: "SYM-030".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::Medium,
            title: "Missing PHPUnit configuration".to_string(),
            description: "No phpunit.xml.dist or phpunit.xml found.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create phpunit.xml.dist with your test configuration".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_missing_tests_dir(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("tests").is_dir() {
        issues.push(Issue {
            id: "SYM-031".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::High,
            title: "Missing tests/ directory".to_string(),
            description: "No tests/ directory found. Symfony projects should have automated tests.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create a tests/ directory and add your first test case".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_missing_phpunit_dep(composer: &ComposerJson, path: &Path, issues: &mut Vec<Issue>) {
    let has_phpunit = composer.has_require_dev("phpunit/phpunit")
        || composer.has_require_dev("symfony/phpunit-bridge")
        || composer.has_require("phpunit/phpunit")
        || composer.has_require("symfony/phpunit-bridge");

    if !has_phpunit {
        issues.push(Issue {
            id: "SYM-032".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::High,
            title: "Missing PHPUnit dependency".to_string(),
            description: "Neither phpunit/phpunit nor symfony/phpunit-bridge found in composer.json.".to_string(),
            file: Some(path.join("composer.json")),
            line: None,
            suggestion: Some("Run `composer require --dev symfony/phpunit-bridge`".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Security checks
// ---------------------------------------------------------------------------

fn check_hardcoded_db_credentials(path: &Path, issues: &mut Vec<Issue>) {
    let env_path = path.join(".env");
    let content = match std::fs::read_to_string(&env_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let re = Regex::new(r"DATABASE_URL\s*=\s*\S+://\w+:.+@").unwrap();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if re.is_match(trimmed) {
            issues.push(Issue {
                id: "SYM-040".to_string(),
                analyzer: "symfony".to_string(),
                category: AnalyzerCategory::Security,
                severity: Severity::Critical,
                title: "Hardcoded database credentials in .env".to_string(),
                description: "DATABASE_URL contains inline credentials (user:pass@). Use environment variables in production.".to_string(),
                file: Some(env_path.clone()),
                line: None,
                suggestion: Some("Use environment variables or a secrets vault for database credentials".to_string()),
                auto_fixable: false,
                references: vec![],
            });
            break;
        }
    }
}

fn check_missing_cors_bundle(composer: &ComposerJson, path: &Path, issues: &mut Vec<Issue>) {
    // Only flag if the project has controllers (i.e., likely serves HTTP)
    if !path.join("src/Controller").is_dir() {
        return;
    }

    if !composer.has_require("nelmio/cors-bundle") {
        issues.push(Issue {
            id: "SYM-041".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Security,
            severity: Severity::Medium,
            title: "Missing CORS bundle".to_string(),
            description: "nelmio/cors-bundle is not installed. API projects need CORS configuration.".to_string(),
            file: Some(path.join("composer.json")),
            line: None,
            suggestion: Some("Run `composer require nelmio/cors-bundle`".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_unserialize_calls(path: &Path, issues: &mut Vec<Issue>) {
    let src_dir = path.join("src");
    if !src_dir.is_dir() {
        return;
    }

    let re = Regex::new(r"unserialize\s*\(").unwrap();

    for entry in WalkDir::new(&src_dir)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                return !SKIP_DIRS.iter().any(|d| name.as_ref() == *d);
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if !name.ends_with(".php") {
            continue;
        }

        let file_path = entry.into_path();
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    issues.push(Issue {
                        id: "SYM-042".to_string(),
                        analyzer: "symfony".to_string(),
                        category: AnalyzerCategory::Security,
                        severity: Severity::Critical,
                        title: "Unsafe unserialize() call".to_string(),
                        description: format!(
                            "unserialize() found in {}. This can lead to object injection vulnerabilities.",
                            file_path.display()
                        ),
                        file: Some(file_path.clone()),
                        line: Some(line_num + 1),
                        suggestion: Some("Use json_decode() or Symfony Serializer instead of unserialize()".to_string()),
                        auto_fixable: false,
                        references: vec![],
                    });
                    break; // One issue per file is enough
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Best practices checks
// ---------------------------------------------------------------------------

fn check_gitignore_entries(path: &Path, issues: &mut Vec<Issue>) {
    let gitignore_path = path.join(".gitignore");
    let content = match std::fs::read_to_string(&gitignore_path) {
        Ok(c) => c,
        Err(_) => return, // No .gitignore is already flagged by StructureAnalyzer
    };

    let has_var = content
        .lines()
        .any(|l| {
            let t = l.trim();
            t == "var/" || t == "/var/" || t == "var"
        });
    let has_vendor = content
        .lines()
        .any(|l| {
            let t = l.trim();
            t == "vendor/" || t == "/vendor/" || t == "vendor"
        });

    let mut missing = Vec::new();
    if !has_var {
        missing.push("var/");
    }
    if !has_vendor {
        missing.push("vendor/");
    }

    if !missing.is_empty() {
        issues.push(Issue {
            id: "SYM-050".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: format!(".gitignore missing: {}", missing.join(", ")),
            description: format!(
                ".gitignore should include {} for Symfony projects.",
                missing.join(" and ")
            ),
            file: Some(gitignore_path),
            line: None,
            suggestion: Some(format!("Add {} to .gitignore", missing.join(" and "))),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_missing_rector(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("rector.php").exists() {
        issues.push(Issue {
            id: "SYM-052".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Info,
            title: "Missing rector.php".to_string(),
            description: "Rector automates code upgrades and refactoring for PHP/Symfony projects.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Run `composer require --dev rector/rector` and create rector.php".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_missing_phpstan(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("phpstan.neon").exists() && !path.join("phpstan.neon.dist").exists() {
        issues.push(Issue {
            id: "SYM-053".to_string(),
            analyzer: "symfony".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Medium,
            title: "Missing PHPStan configuration".to_string(),
            description: "No phpstan.neon or phpstan.neon.dist found. Static analysis catches bugs early.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Run `composer require --dev phpstan/phpstan` and create phpstan.neon".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::detector::{DetectedProject, Language, PackageManager};
    use std::fs as stdfs;
    use tempfile::TempDir;

    fn make_project(tmp: &TempDir) -> Project {
        Project {
            path: tmp.path().to_path_buf(),
            detected: DetectedProject {
                framework: Framework::Symfony,
                language: Language::Php,
                version: None,
                package_manager: Some(PackageManager::Composer),
                has_git: false,
                has_ci: None,
            },
        }
    }

    /// Minimal Symfony scaffold: src/Controller, src/Entity, tests, .gitignore,
    /// phpunit.xml.dist, composer.json with phpunit-bridge and symfony/runtime.
    fn scaffold_symfony(tmp: &TempDir) {
        stdfs::create_dir_all(tmp.path().join("src/Controller")).unwrap();
        stdfs::create_dir_all(tmp.path().join("src/Entity")).unwrap();
        stdfs::create_dir_all(tmp.path().join("tests")).unwrap();
        stdfs::write(
            tmp.path().join(".gitignore"),
            "var/\nvendor/\n.env\n",
        )
        .unwrap();
        stdfs::write(tmp.path().join("phpunit.xml.dist"), "<phpunit/>").unwrap();
        stdfs::write(tmp.path().join("rector.php"), "<?php").unwrap();
        stdfs::write(tmp.path().join("phpstan.neon"), "parameters:").unwrap();
        stdfs::write(
            tmp.path().join("composer.json"),
            r#"{
                "require": {
                    "symfony/framework-bundle": "^7.0",
                    "symfony/runtime": "^7.0",
                    "nelmio/cors-bundle": "^2.0"
                },
                "require-dev": {
                    "symfony/phpunit-bridge": "^7.0"
                }
            }"#,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_applies_only_to_symfony() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);
        assert!(SymfonyAnalyzer.applies_to(&project));

        let non_symfony = Project {
            path: tmp.path().to_path_buf(),
            detected: DetectedProject {
                framework: Framework::RustCargo,
                language: Language::Rust,
                version: None,
                package_manager: Some(PackageManager::Cargo),
                has_git: false,
                has_ci: None,
            },
        };
        assert!(!SymfonyAnalyzer.applies_to(&non_symfony));
    }

    #[tokio::test]
    async fn test_clean_symfony_project() {
        let tmp = TempDir::new().unwrap();
        scaffold_symfony(&tmp);
        let project = make_project(&tmp);
        let issues = SymfonyAnalyzer.analyze(&project).await.unwrap();
        // A well-scaffolded project should produce no issues
        assert!(
            issues.is_empty(),
            "Expected no issues but got: {:?}",
            issues.iter().map(|i| &i.id).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_missing_controller_and_entity_dirs() {
        let tmp = TempDir::new().unwrap();
        stdfs::create_dir_all(tmp.path().join("src")).unwrap();
        let project = make_project(&tmp);
        let issues = SymfonyAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SYM-001"));
        assert!(issues.iter().any(|i| i.id == "SYM-002"));
    }

    #[tokio::test]
    async fn test_misplaced_controller() {
        let tmp = TempDir::new().unwrap();
        scaffold_symfony(&tmp);
        stdfs::create_dir_all(tmp.path().join("src/Other")).unwrap();
        stdfs::write(
            tmp.path().join("src/Other/FooController.php"),
            "<?php class FooController {}",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SymfonyAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SYM-003"));
    }

    #[tokio::test]
    async fn test_misplaced_service() {
        let tmp = TempDir::new().unwrap();
        scaffold_symfony(&tmp);
        stdfs::create_dir_all(tmp.path().join("src/Other")).unwrap();
        stdfs::write(
            tmp.path().join("src/Other/MailService.php"),
            "<?php class MailService {}",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SymfonyAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SYM-004"));
    }

    #[tokio::test]
    async fn test_weak_app_secret() {
        let tmp = TempDir::new().unwrap();
        scaffold_symfony(&tmp);
        stdfs::write(tmp.path().join(".env"), "APP_SECRET=change_me\n").unwrap();
        let project = make_project(&tmp);
        let issues = SymfonyAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SYM-012"));
    }

    #[tokio::test]
    async fn test_prod_debug_enabled() {
        let tmp = TempDir::new().unwrap();
        scaffold_symfony(&tmp);
        stdfs::create_dir_all(tmp.path().join("config/packages/prod")).unwrap();
        stdfs::write(
            tmp.path().join("config/packages/prod/framework.yaml"),
            "framework:\n    debug: true\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SymfonyAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SYM-013"));
    }

    #[tokio::test]
    async fn test_outdated_symfony_version() {
        let tmp = TempDir::new().unwrap();
        scaffold_symfony(&tmp);
        stdfs::write(
            tmp.path().join("composer.json"),
            r#"{
                "require": {
                    "symfony/framework-bundle": "^5.4",
                    "symfony/runtime": "^5.4",
                    "nelmio/cors-bundle": "^2.0"
                },
                "require-dev": {
                    "symfony/phpunit-bridge": "^5.4"
                }
            }"#,
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SymfonyAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SYM-020"));
    }

    #[tokio::test]
    async fn test_missing_tests_and_phpunit() {
        let tmp = TempDir::new().unwrap();
        stdfs::create_dir_all(tmp.path().join("src/Controller")).unwrap();
        stdfs::create_dir_all(tmp.path().join("src/Entity")).unwrap();
        stdfs::write(
            tmp.path().join("composer.json"),
            r#"{"require":{"symfony/framework-bundle":"^7.0","symfony/runtime":"^7.0","nelmio/cors-bundle":"^2.0"},"require-dev":{}}"#,
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SymfonyAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SYM-030"));
        assert!(issues.iter().any(|i| i.id == "SYM-031"));
        assert!(issues.iter().any(|i| i.id == "SYM-032"));
    }

    #[tokio::test]
    async fn test_hardcoded_db_credentials() {
        let tmp = TempDir::new().unwrap();
        scaffold_symfony(&tmp);
        stdfs::write(
            tmp.path().join(".env"),
            "APP_SECRET=a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4\nDATABASE_URL=mysql://root:secret@127.0.0.1:3306/mydb\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SymfonyAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SYM-040"));
    }

    #[tokio::test]
    async fn test_unserialize_call() {
        let tmp = TempDir::new().unwrap();
        scaffold_symfony(&tmp);
        stdfs::write(
            tmp.path().join("src/Controller/BadController.php"),
            "<?php\n$data = unserialize($input);\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SymfonyAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SYM-042"));
    }

    #[tokio::test]
    async fn test_gitignore_missing_entries() {
        let tmp = TempDir::new().unwrap();
        scaffold_symfony(&tmp);
        // Overwrite .gitignore without var/ and vendor/
        stdfs::write(tmp.path().join(".gitignore"), ".env\n").unwrap();
        let project = make_project(&tmp);
        let issues = SymfonyAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SYM-050"));
    }

    #[tokio::test]
    async fn test_parse_symfony_major_version() {
        assert_eq!(parse_symfony_major_version("^7.0"), Some(7));
        assert_eq!(parse_symfony_major_version("~6.4"), Some(6));
        assert_eq!(parse_symfony_major_version(">=5.4"), Some(5));
        assert_eq!(parse_symfony_major_version("5.4.*"), Some(5));
        assert_eq!(parse_symfony_major_version("^7.1.2"), Some(7));
        assert_eq!(parse_symfony_major_version("invalid"), None);
    }
}
