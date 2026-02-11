use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

use crate::analyzers::traits::{Analyzer, AnalyzerCategory, Issue, Severity};
use crate::core::project::Project;
use crate::frameworks::detector::Framework;
use crate::utils::fs::find_files_with_extension;

pub struct LaravelAnalyzer;

/// Parsed subset of composer.json relevant to Laravel checks.
struct ComposerJson {
    require: HashMap<String, String>,
}

impl ComposerJson {
    fn parse(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path.join("composer.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;

        let require = Self::parse_dep_map(json.get("require"));

        Some(Self { require })
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
}

/// Directories to skip when walking the project tree.
const SKIP_DIRS: &[&str] = &["vendor", "var", ".git", "node_modules", "storage"];

#[async_trait]
impl Analyzer for LaravelAnalyzer {
    fn name(&self) -> &'static str {
        "laravel"
    }

    fn description(&self) -> &'static str {
        "Laravel-specific project structure, configuration, and best practices"
    }

    fn category(&self) -> AnalyzerCategory {
        AnalyzerCategory::Structure
    }

    fn applies_to(&self, project: &Project) -> bool {
        project.detected.framework == Framework::Laravel
    }

    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let path = &project.path;
        let composer = ComposerJson::parse(path);

        // Structure checks
        check_missing_controllers_dir(path, &mut issues);
        check_missing_routes_dir(path, &mut issues);
        check_missing_views_dir(path, &mut issues);

        // Configuration checks
        check_default_app_key(path, &mut issues);
        check_debug_mode(path, &mut issues);

        // Dependency checks
        if let Some(ref c) = composer {
            check_dev_deps_in_require(c, path, &mut issues);
        }

        // Testing checks
        check_missing_phpunit_config(path, &mut issues);
        check_missing_tests_dir(path, &mut issues);

        // Security checks
        check_unguarded_models(path, &mut issues);
        check_raw_sql_queries(path, &mut issues);

        // Best practices
        check_gitignore_entries(path, &mut issues);

        Ok(issues)
    }
}

// ---------------------------------------------------------------------------
// Structure checks
// ---------------------------------------------------------------------------

fn check_missing_controllers_dir(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("app/Http/Controllers").is_dir() {
        issues.push(Issue {
            id: "LAR-001".to_string(),
            analyzer: "laravel".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::High,
            title: "Missing app/Http/Controllers/ directory".to_string(),
            description: "Laravel projects should have an app/Http/Controllers/ directory for HTTP controllers.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create app/Http/Controllers/ and add your first controller".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_missing_routes_dir(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("routes").is_dir() {
        issues.push(Issue {
            id: "LAR-002".to_string(),
            analyzer: "laravel".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: "Missing routes/ directory".to_string(),
            description: "Laravel projects should have a routes/ directory for route definitions.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create routes/ directory with web.php and api.php".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_missing_views_dir(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("resources/views").is_dir() {
        issues.push(Issue {
            id: "LAR-003".to_string(),
            analyzer: "laravel".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: "Missing resources/views/ directory".to_string(),
            description: "Laravel projects should have a resources/views/ directory for Blade templates.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create resources/views/ for your Blade templates".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Configuration checks
// ---------------------------------------------------------------------------

fn check_default_app_key(path: &Path, issues: &mut Vec<Issue>) {
    let env_path = path.join(".env");
    let content = match std::fs::read_to_string(&env_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("APP_KEY=") {
            continue;
        }
        let value = trimmed.trim_start_matches("APP_KEY=").trim();
        if value.is_empty() || value == "base64:" || value == "SomeRandomString" {
            issues.push(Issue {
                id: "LAR-010".to_string(),
                analyzer: "laravel".to_string(),
                category: AnalyzerCategory::Configuration,
                severity: Severity::Critical,
                title: "Default or empty APP_KEY".to_string(),
                description: "APP_KEY in .env is empty or a known default. Run `php artisan key:generate`.".to_string(),
                file: Some(env_path.clone()),
                line: None,
                suggestion: Some("Run `php artisan key:generate` to set a secure application key".to_string()),
                auto_fixable: false,
                references: vec![],
            });
        }
        break;
    }
}

fn check_debug_mode(path: &Path, issues: &mut Vec<Issue>) {
    let env_path = path.join(".env");
    let content = match std::fs::read_to_string(&env_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("APP_DEBUG=true") {
            issues.push(Issue {
                id: "LAR-011".to_string(),
                analyzer: "laravel".to_string(),
                category: AnalyzerCategory::Configuration,
                severity: Severity::High,
                title: "Debug mode enabled in .env".to_string(),
                description: "APP_DEBUG=true in .env. Ensure this is disabled in production.".to_string(),
                file: Some(env_path.clone()),
                line: None,
                suggestion: Some("Set APP_DEBUG=false in production .env".to_string()),
                auto_fixable: false,
                references: vec![],
            });
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Dependency checks
// ---------------------------------------------------------------------------

fn check_dev_deps_in_require(composer: &ComposerJson, path: &Path, issues: &mut Vec<Issue>) {
    let dev_packages = [
        "phpunit/phpunit",
        "fakerphp/faker",
        "mockery/mockery",
        "laravel/sail",
        "laravel/pint",
    ];

    for pkg in &dev_packages {
        if composer.require.contains_key(*pkg) {
            issues.push(Issue {
                id: "LAR-020".to_string(),
                analyzer: "laravel".to_string(),
                category: AnalyzerCategory::Dependencies,
                severity: Severity::Medium,
                title: format!("Dev dependency in require section: {}", pkg),
                description: format!(
                    "{} is in require but should be in require-dev.",
                    pkg
                ),
                file: Some(path.join("composer.json")),
                line: None,
                suggestion: Some(format!("Move {} to require-dev section", pkg)),
                auto_fixable: false,
                references: vec![],
            });
            break; // Report once
        }
    }
}

// ---------------------------------------------------------------------------
// Testing checks
// ---------------------------------------------------------------------------

fn check_missing_phpunit_config(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("phpunit.xml").exists() && !path.join("phpunit.xml.dist").exists() {
        issues.push(Issue {
            id: "LAR-030".to_string(),
            analyzer: "laravel".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::High,
            title: "Missing PHPUnit configuration".to_string(),
            description: "No phpunit.xml or phpunit.xml.dist found. Laravel ships with PHPUnit by default.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create phpunit.xml with your test configuration".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_missing_tests_dir(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("tests").is_dir() {
        issues.push(Issue {
            id: "LAR-031".to_string(),
            analyzer: "laravel".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::High,
            title: "Missing tests/ directory".to_string(),
            description: "No tests/ directory found. Laravel projects should have automated tests.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create a tests/ directory with Feature and Unit subdirectories".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Security checks
// ---------------------------------------------------------------------------

fn check_unguarded_models(path: &Path, issues: &mut Vec<Issue>) {
    let models_dir = path.join("app/Models");
    if !models_dir.is_dir() {
        return;
    }

    let extends_re = Regex::new(r"extends\s+Model").unwrap();
    let guarded_re = Regex::new(r"\$(fillable|guarded)\s*=").unwrap();

    for file_path in find_files_with_extension(&models_dir, "php") {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            if extends_re.is_match(&content) && !guarded_re.is_match(&content) {
                issues.push(Issue {
                    id: "LAR-040".to_string(),
                    analyzer: "laravel".to_string(),
                    category: AnalyzerCategory::Security,
                    severity: Severity::High,
                    title: "Unguarded model (mass assignment risk)".to_string(),
                    description: format!(
                        "Model {} extends Model without $fillable or $guarded property.",
                        file_path.display()
                    ),
                    file: Some(file_path),
                    line: None,
                    suggestion: Some("Add $fillable or $guarded property to protect against mass assignment".to_string()),
                    auto_fixable: false,
                    references: vec![],
                });
            }
        }
    }
}

fn check_raw_sql_queries(path: &Path, issues: &mut Vec<Issue>) {
    let re = Regex::new(r"(DB::raw\(|->whereRaw\(|->selectRaw\()").unwrap();
    let _ = SKIP_DIRS; // used conceptually via find_files_with_extension

    for file_path in find_files_with_extension(path, "php") {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    issues.push(Issue {
                        id: "LAR-041".to_string(),
                        analyzer: "laravel".to_string(),
                        category: AnalyzerCategory::Security,
                        severity: Severity::High,
                        title: "Raw SQL query detected".to_string(),
                        description: format!(
                            "Raw SQL usage found in {}. This may be vulnerable to SQL injection.",
                            file_path.display()
                        ),
                        file: Some(file_path.clone()),
                        line: Some(line_num + 1),
                        suggestion: Some("Use Eloquent query builder or parameterized queries instead of raw SQL".to_string()),
                        auto_fixable: false,
                        references: vec![],
                    });
                    break; // One issue per file
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Best practices
// ---------------------------------------------------------------------------

fn check_gitignore_entries(path: &Path, issues: &mut Vec<Issue>) {
    let gitignore_path = path.join(".gitignore");
    let content = match std::fs::read_to_string(&gitignore_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let has_vendor = content.lines().any(|l| {
        let t = l.trim();
        t == "vendor/" || t == "/vendor/" || t == "vendor"
    });
    let has_env = content.lines().any(|l| {
        let t = l.trim();
        t == ".env" || t == "/.env"
    });

    let mut missing = Vec::new();
    if !has_vendor {
        missing.push("vendor/");
    }
    if !has_env {
        missing.push(".env");
    }

    if !missing.is_empty() {
        issues.push(Issue {
            id: "LAR-050".to_string(),
            analyzer: "laravel".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: format!(".gitignore missing: {}", missing.join(", ")),
            description: format!(
                ".gitignore should include {} for Laravel projects.",
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
                framework: Framework::Laravel,
                language: Language::Php,
                version: None,
                package_manager: Some(PackageManager::Composer),
                has_git: false,
                has_ci: None,
            },
        }
    }

    /// Minimal Laravel scaffold: all dirs, phpunit, .gitignore, .env with key, composer.json clean.
    fn scaffold_laravel(tmp: &TempDir) {
        stdfs::create_dir_all(tmp.path().join("app/Http/Controllers")).unwrap();
        stdfs::create_dir_all(tmp.path().join("app/Models")).unwrap();
        stdfs::create_dir_all(tmp.path().join("routes")).unwrap();
        stdfs::create_dir_all(tmp.path().join("resources/views")).unwrap();
        stdfs::create_dir_all(tmp.path().join("tests/Feature")).unwrap();
        stdfs::create_dir_all(tmp.path().join("tests/Unit")).unwrap();
        stdfs::write(
            tmp.path().join(".gitignore"),
            "vendor/\n.env\nnode_modules/\n",
        )
        .unwrap();
        stdfs::write(
            tmp.path().join(".env"),
            "APP_KEY=base64:abc123def456ghi789jkl012mno345pq\nAPP_DEBUG=false\n",
        )
        .unwrap();
        stdfs::write(tmp.path().join("phpunit.xml"), "<phpunit/>").unwrap();
        stdfs::write(
            tmp.path().join("composer.json"),
            r#"{
                "require": {
                    "laravel/framework": "^11.0"
                },
                "require-dev": {
                    "phpunit/phpunit": "^10.0",
                    "fakerphp/faker": "^1.0"
                }
            }"#,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_applies_only_to_laravel() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);
        assert!(LaravelAnalyzer.applies_to(&project));

        let non_laravel = Project {
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
        assert!(!LaravelAnalyzer.applies_to(&non_laravel));
    }

    #[tokio::test]
    async fn test_clean_laravel_project() {
        let tmp = TempDir::new().unwrap();
        scaffold_laravel(&tmp);
        let project = make_project(&tmp);
        let issues = LaravelAnalyzer.analyze(&project).await.unwrap();
        assert!(
            issues.is_empty(),
            "Expected no issues but got: {:?}",
            issues.iter().map(|i| &i.id).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_missing_structure_dirs() {
        let tmp = TempDir::new().unwrap();
        // Empty project - no dirs at all
        let project = make_project(&tmp);
        let issues = LaravelAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "LAR-001"));
        assert!(issues.iter().any(|i| i.id == "LAR-002"));
        assert!(issues.iter().any(|i| i.id == "LAR-003"));
    }

    #[tokio::test]
    async fn test_default_app_key() {
        let tmp = TempDir::new().unwrap();
        scaffold_laravel(&tmp);
        stdfs::write(tmp.path().join(".env"), "APP_KEY=\nAPP_DEBUG=false\n").unwrap();
        let project = make_project(&tmp);
        let issues = LaravelAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "LAR-010"));
    }

    #[tokio::test]
    async fn test_mass_assignment_detection() {
        let tmp = TempDir::new().unwrap();
        scaffold_laravel(&tmp);
        stdfs::write(
            tmp.path().join("app/Models/User.php"),
            "<?php\nclass User extends Model\n{\n    // no fillable or guarded\n}\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = LaravelAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "LAR-040"));
    }

    #[tokio::test]
    async fn test_raw_sql_detection() {
        let tmp = TempDir::new().unwrap();
        scaffold_laravel(&tmp);
        stdfs::create_dir_all(tmp.path().join("app/Http/Controllers")).unwrap();
        stdfs::write(
            tmp.path().join("app/Http/Controllers/UserController.php"),
            "<?php\n$users = DB::raw('SELECT * FROM users');\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = LaravelAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "LAR-041"));
    }
}
