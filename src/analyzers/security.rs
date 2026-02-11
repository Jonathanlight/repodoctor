use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::analyzers::traits::{Analyzer, AnalyzerCategory, Issue, Severity};
use crate::core::project::Project;
use crate::utils::fs::path_exists;

pub struct SecurityAnalyzer;

const MAX_FILES: usize = 500;
const MAX_LINES: usize = 1000;

/// File extensions to scan for secrets.
const SCANNABLE_EXTENSIONS: &[&str] = &[
    "env", "yml", "yaml", "json", "toml", "php", "js", "ts", "py", "rs", "dart", "rb", "go",
    "cfg", "ini", "conf", "properties",
];

/// Directories to skip during scanning.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "vendor",
    "target",
    ".git",
    ".svn",
    "__pycache__",
    ".tox",
    "dist",
    "build",
];

/// File names to skip (lock files etc.).
const SKIP_FILES: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Cargo.lock",
    "composer.lock",
    "pubspec.lock",
    "poetry.lock",
    "Gemfile.lock",
];

struct SecretPattern {
    name: &'static str,
    regex: &'static str,
}

const SECRET_PATTERNS: &[SecretPattern] = &[
    SecretPattern {
        name: "API key",
        regex: r#"(?i)(api[_\-]?key|apikey)["']?\s*[=:]\s*["']?[a-zA-Z0-9]{16,}"#,
    },
    SecretPattern {
        name: "Password",
        regex: r#"(?i)(password|passwd|pwd)\s*[=:]\s*["'][^"']{4,}["']"#,
    },
    SecretPattern {
        name: "Secret/Token",
        regex: r#"(?i)(secret|token|auth)\s*[=:]\s*["'][^"']{8,}["']"#,
    },
    SecretPattern {
        name: "AWS Access Key",
        regex: r"AKIA[0-9A-Z]{16}",
    },
    SecretPattern {
        name: "Private key",
        regex: r"-----BEGIN (RSA |EC |DSA )?PRIVATE KEY-----",
    },
];

#[async_trait]
impl Analyzer for SecurityAnalyzer {
    fn name(&self) -> &'static str {
        "security"
    }

    fn description(&self) -> &'static str {
        "Scans for potential secrets, credentials, and security issues"
    }

    fn category(&self) -> AnalyzerCategory {
        AnalyzerCategory::Security
    }

    fn applies_to(&self, _project: &Project) -> bool {
        true
    }

    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let path = &project.path;

        // SEC-003: .env without .gitignore entry
        check_env_gitignore(path, &mut issues);

        // SEC-001 / SEC-002: Scan files for secrets
        scan_for_secrets(path, &mut issues)?;

        Ok(issues)
    }
}

fn check_env_gitignore(path: &Path, issues: &mut Vec<Issue>) {
    if !path_exists(path, ".env") {
        return;
    }

    let gitignore_path = path.join(".gitignore");
    let is_gitignored = if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
        content.lines().any(|line| {
            let trimmed = line.trim();
            trimmed == ".env" || trimmed == "/.env" || trimmed == ".env*"
        })
    } else {
        false
    };

    if !is_gitignored {
        issues.push(Issue {
            id: "SEC-003".to_string(),
            analyzer: "security".to_string(),
            category: AnalyzerCategory::Security,
            severity: Severity::High,
            title: ".env file without .gitignore entry".to_string(),
            description: ".env file exists but is not listed in .gitignore. Secrets may be committed to version control.".to_string(),
            file: Some(path.join(".env")),
            line: None,
            suggestion: Some("Add .env to .gitignore".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn scan_for_secrets(path: &Path, issues: &mut Vec<Issue>) -> Result<()> {
    let compiled: Vec<(&str, Regex)> = SECRET_PATTERNS
        .iter()
        .filter_map(|p| Regex::new(p.regex).ok().map(|r| (p.name, r)))
        .collect();

    let files = collect_scannable_files(path);

    for file_path in files {
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Check for private key files
        if content.contains("-----BEGIN") && content.contains("PRIVATE KEY-----") {
            issues.push(Issue {
                id: "SEC-002".to_string(),
                analyzer: "security".to_string(),
                category: AnalyzerCategory::Security,
                severity: Severity::Critical,
                title: "Private key file detected".to_string(),
                description: format!(
                    "File appears to contain a private key: {}",
                    file_path.display()
                ),
                file: Some(file_path.clone()),
                line: None,
                suggestion: Some(
                    "Remove private keys from the repository and use a secrets manager"
                        .to_string(),
                ),
                auto_fixable: false,
                references: vec![],
            });
            continue; // Don't double-report on this file
        }

        for (line_num, line) in content.lines().enumerate().take(MAX_LINES) {
            for (name, regex) in &compiled {
                if regex.is_match(line) {
                    issues.push(Issue {
                        id: "SEC-001".to_string(),
                        analyzer: "security".to_string(),
                        category: AnalyzerCategory::Security,
                        severity: Severity::Critical,
                        title: format!("Potential {} found", name),
                        description: format!(
                            "Possible {} detected in {}",
                            name,
                            file_path.display()
                        ),
                        file: Some(file_path.clone()),
                        line: Some(line_num + 1),
                        suggestion: Some(
                            "Remove credentials and use environment variables or a secrets manager"
                                .to_string(),
                        ),
                        auto_fixable: false,
                        references: vec![],
                    });
                    break; // One issue per line is enough
                }
            }
        }
    }

    Ok(())
}

fn collect_scannable_files(path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            if e.file_type().is_dir() {
                return !SKIP_DIRS.iter().any(|d| name.as_ref() == *d);
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if files.len() >= MAX_FILES {
            break;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let file_name = entry.file_name().to_string_lossy();

        // Skip lock files
        if SKIP_FILES.iter().any(|f| file_name.as_ref() == *f) {
            continue;
        }

        // Check extension
        if let Some(ext) = entry.path().extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if SCANNABLE_EXTENSIONS.contains(&ext_str.as_str()) {
                files.push(entry.into_path());
            }
        }
    }

    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::detector::{DetectedProject, Framework, Language};
    use std::fs as stdfs;
    use tempfile::TempDir;

    fn make_project(tmp: &TempDir) -> Project {
        Project {
            path: tmp.path().to_path_buf(),
            detected: DetectedProject {
                framework: Framework::Unknown,
                language: Language::Unknown,
                version: None,
                package_manager: None,
                has_git: false,
                has_ci: None,
            },
        }
    }

    #[tokio::test]
    async fn test_env_without_gitignore() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join(".env"), "SECRET=value").unwrap();
        let project = make_project(&tmp);
        let issues = SecurityAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SEC-003"));
    }

    #[tokio::test]
    async fn test_env_with_gitignore() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join(".env"), "SECRET=value").unwrap();
        stdfs::write(tmp.path().join(".gitignore"), ".env\n").unwrap();
        let project = make_project(&tmp);
        let issues = SecurityAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "SEC-003"));
    }

    #[tokio::test]
    async fn test_detect_api_key() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(
            tmp.path().join("config.json"),
            r#"{"api_key": "abcdef1234567890abcdef"}"#,
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SecurityAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SEC-001"));
    }

    #[tokio::test]
    async fn test_detect_password() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(
            tmp.path().join("app.yaml"),
            "password: 'mysecretpassword123'",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SecurityAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SEC-001"));
    }

    #[tokio::test]
    async fn test_detect_private_key() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(
            tmp.path().join("key.pem"),
            "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAK...\n-----END RSA PRIVATE KEY-----",
        )
        .unwrap();
        // .pem not in scannable extensions, put it in a .json for test
        stdfs::write(
            tmp.path().join("secrets.json"),
            "-----BEGIN RSA PRIVATE KEY-----\ndata\n-----END RSA PRIVATE KEY-----",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SecurityAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SEC-002"));
    }

    #[tokio::test]
    async fn test_detect_aws_key() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(
            tmp.path().join("config.yaml"),
            "aws_key: AKIAIOSFODNN7EXAMPLE",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SecurityAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "SEC-001"));
    }

    #[tokio::test]
    async fn test_no_secrets_clean_project() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join("main.rs"), "fn main() {}").unwrap();
        let project = make_project(&tmp);
        let issues = SecurityAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "SEC-001" || i.id == "SEC-002"));
    }

    #[tokio::test]
    async fn test_skips_lock_files() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(
            tmp.path().join("package-lock.json"),
            r#"{"api_key": "abcdef1234567890abcdef"}"#,
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = SecurityAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "SEC-001"));
    }

    #[tokio::test]
    async fn test_applies_to_all() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);
        assert!(SecurityAnalyzer.applies_to(&project));
    }
}
