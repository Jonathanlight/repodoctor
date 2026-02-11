use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use std::path::Path;

use crate::analyzers::traits::{Analyzer, AnalyzerCategory, Issue, Severity};
use crate::core::project::Project;
use crate::frameworks::detector::Framework;
use crate::utils::fs::find_files_with_extension;

pub struct RustCargoAnalyzer;

#[async_trait]
impl Analyzer for RustCargoAnalyzer {
    fn name(&self) -> &'static str {
        "rust_cargo"
    }

    fn description(&self) -> &'static str {
        "Rust/Cargo-specific project structure, configuration, and best practices"
    }

    fn category(&self) -> AnalyzerCategory {
        AnalyzerCategory::Structure
    }

    fn applies_to(&self, project: &Project) -> bool {
        project.detected.framework == Framework::RustCargo
    }

    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let path = &project.path;

        // Structure checks
        check_missing_entry_point(path, &mut issues);
        check_missing_clippy_config(path, &mut issues);
        check_missing_rustfmt_config(path, &mut issues);

        // Configuration checks
        check_outdated_edition(path, &mut issues);
        check_missing_cargo_lock(path, &mut issues);

        // Testing checks
        check_missing_tests_dir(path, &mut issues);

        // Security checks
        check_unsafe_blocks(path, &mut issues);

        // Best practices
        check_gitignore_entries(path, &mut issues);

        Ok(issues)
    }
}

// ---------------------------------------------------------------------------
// Structure checks
// ---------------------------------------------------------------------------

fn check_missing_entry_point(path: &Path, issues: &mut Vec<Issue>) {
    let has_main = path.join("src/main.rs").exists();
    let has_lib = path.join("src/lib.rs").exists();

    if !has_main && !has_lib {
        issues.push(Issue {
            id: "RST-001".to_string(),
            analyzer: "rust_cargo".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::High,
            title: "Missing src/main.rs or src/lib.rs".to_string(),
            description: "Rust projects need either src/main.rs (binary) or src/lib.rs (library) as an entry point.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create src/main.rs for a binary crate or src/lib.rs for a library crate".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_missing_clippy_config(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("clippy.toml").exists() && !path.join(".clippy.toml").exists() {
        issues.push(Issue {
            id: "RST-002".to_string(),
            analyzer: "rust_cargo".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Low,
            title: "Missing clippy configuration".to_string(),
            description: "No clippy.toml or .clippy.toml found. Clippy configuration helps enforce consistent lint rules.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create clippy.toml to configure Clippy lints for your project".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_missing_rustfmt_config(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("rustfmt.toml").exists() && !path.join(".rustfmt.toml").exists() {
        issues.push(Issue {
            id: "RST-003".to_string(),
            analyzer: "rust_cargo".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Low,
            title: "Missing rustfmt configuration".to_string(),
            description: "No rustfmt.toml or .rustfmt.toml found. A consistent code style helps readability.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create rustfmt.toml to configure code formatting rules".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Configuration checks
// ---------------------------------------------------------------------------

fn check_outdated_edition(path: &Path, issues: &mut Vec<Issue>) {
    let cargo_path = path.join("Cargo.toml");
    let content = match std::fs::read_to_string(&cargo_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let edition_re = Regex::new(r#"edition\s*=\s*"(\d+)""#).unwrap();

    if let Some(caps) = edition_re.captures(&content) {
        if let Ok(year) = caps[1].parse::<u32>() {
            if year < 2021 {
                issues.push(Issue {
                    id: "RST-010".to_string(),
                    analyzer: "rust_cargo".to_string(),
                    category: AnalyzerCategory::Configuration,
                    severity: Severity::Medium,
                    title: format!("Outdated Rust edition ({})", year),
                    description: format!(
                        "Cargo.toml specifies edition {}. Consider upgrading to 2021 or later.",
                        year
                    ),
                    file: Some(cargo_path),
                    line: None,
                    suggestion: Some("Update edition to \"2021\" in Cargo.toml".to_string()),
                    auto_fixable: false,
                    references: vec![],
                });
            }
        }
    } else {
        // No edition specified
        issues.push(Issue {
            id: "RST-010".to_string(),
            analyzer: "rust_cargo".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Medium,
            title: "Missing Rust edition in Cargo.toml".to_string(),
            description: "No edition specified in Cargo.toml. Without it, the 2015 edition is used by default.".to_string(),
            file: Some(cargo_path),
            line: None,
            suggestion: Some("Add edition = \"2021\" to [package] in Cargo.toml".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_missing_cargo_lock(path: &Path, issues: &mut Vec<Issue>) {
    // Only flag for binaries (src/main.rs present) that are missing Cargo.lock
    if path.join("src/main.rs").exists() && !path.join("Cargo.lock").exists() {
        issues.push(Issue {
            id: "RST-011".to_string(),
            analyzer: "rust_cargo".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Medium,
            title: "Missing Cargo.lock for binary crate".to_string(),
            description: "Binary crates should commit Cargo.lock for reproducible builds.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Run `cargo build` and commit the generated Cargo.lock".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Testing checks
// ---------------------------------------------------------------------------

fn check_missing_tests_dir(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("tests").is_dir() {
        issues.push(Issue {
            id: "RST-020".to_string(),
            analyzer: "rust_cargo".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::Medium,
            title: "No integration tests directory".to_string(),
            description: "No tests/ directory found. Consider adding integration tests.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create a tests/ directory for integration tests".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Security checks
// ---------------------------------------------------------------------------

fn check_unsafe_blocks(path: &Path, issues: &mut Vec<Issue>) {
    let src_dir = path.join("src");
    if !src_dir.is_dir() {
        return;
    }

    let re = Regex::new(r"unsafe\s*\{").unwrap();

    for file_path in find_files_with_extension(&src_dir, "rs") {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    issues.push(Issue {
                        id: "RST-030".to_string(),
                        analyzer: "rust_cargo".to_string(),
                        category: AnalyzerCategory::Security,
                        severity: Severity::High,
                        title: "Unsafe code block detected".to_string(),
                        description: format!(
                            "unsafe block found in {}. Ensure unsafe code is justified and reviewed.",
                            file_path.display()
                        ),
                        file: Some(file_path.clone()),
                        line: Some(line_num + 1),
                        suggestion: Some("Review unsafe code for soundness or replace with safe alternatives".to_string()),
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

    let has_target = content.lines().any(|l| {
        let t = l.trim();
        t == "target/" || t == "/target/" || t == "target"
    });

    if !has_target {
        issues.push(Issue {
            id: "RST-040".to_string(),
            analyzer: "rust_cargo".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: ".gitignore missing: target/".to_string(),
            description: ".gitignore should include target/ for Rust projects.".to_string(),
            file: Some(gitignore_path),
            line: None,
            suggestion: Some("Add target/ to .gitignore".to_string()),
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
                framework: Framework::RustCargo,
                language: Language::Rust,
                version: None,
                package_manager: Some(PackageManager::Cargo),
                has_git: false,
                has_ci: None,
            },
        }
    }

    /// Minimal Rust/Cargo scaffold: src/main.rs, Cargo.toml with edition 2021,
    /// Cargo.lock, tests/, .gitignore with target/, clippy.toml, rustfmt.toml.
    fn scaffold_rust(tmp: &TempDir) {
        stdfs::create_dir_all(tmp.path().join("src")).unwrap();
        stdfs::create_dir_all(tmp.path().join("tests")).unwrap();
        stdfs::write(
            tmp.path().join("src/main.rs"),
            "fn main() {\n    println!(\"Hello, world!\");\n}\n",
        )
        .unwrap();
        stdfs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        stdfs::write(tmp.path().join("Cargo.lock"), "# lock file\n").unwrap();
        stdfs::write(tmp.path().join(".gitignore"), "target/\n").unwrap();
        stdfs::write(tmp.path().join("clippy.toml"), "").unwrap();
        stdfs::write(tmp.path().join("rustfmt.toml"), "").unwrap();
    }

    #[tokio::test]
    async fn test_applies_only_to_rust() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);
        assert!(RustCargoAnalyzer.applies_to(&project));

        let non_rust = Project {
            path: tmp.path().to_path_buf(),
            detected: DetectedProject {
                framework: Framework::Symfony,
                language: Language::Php,
                version: None,
                package_manager: Some(PackageManager::Composer),
                has_git: false,
                has_ci: None,
            },
        };
        assert!(!RustCargoAnalyzer.applies_to(&non_rust));
    }

    #[tokio::test]
    async fn test_clean_rust_project() {
        let tmp = TempDir::new().unwrap();
        scaffold_rust(&tmp);
        let project = make_project(&tmp);
        let issues = RustCargoAnalyzer.analyze(&project).await.unwrap();
        assert!(
            issues.is_empty(),
            "Expected no issues but got: {:?}",
            issues.iter().map(|i| &i.id).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_missing_entry_point() {
        let tmp = TempDir::new().unwrap();
        stdfs::create_dir_all(tmp.path().join("src")).unwrap();
        stdfs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = RustCargoAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "RST-001"));
    }

    #[tokio::test]
    async fn test_outdated_edition() {
        let tmp = TempDir::new().unwrap();
        scaffold_rust(&tmp);
        stdfs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2018\"\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = RustCargoAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "RST-010"));
    }

    #[tokio::test]
    async fn test_unsafe_detection() {
        let tmp = TempDir::new().unwrap();
        scaffold_rust(&tmp);
        stdfs::write(
            tmp.path().join("src/main.rs"),
            "fn main() {\n    unsafe {\n        // danger\n    }\n}\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = RustCargoAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "RST-030"));
    }

    #[tokio::test]
    async fn test_cargo_lock_for_binary() {
        let tmp = TempDir::new().unwrap();
        scaffold_rust(&tmp);
        // Remove Cargo.lock
        stdfs::remove_file(tmp.path().join("Cargo.lock")).unwrap();
        let project = make_project(&tmp);
        let issues = RustCargoAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "RST-011"));
    }
}
