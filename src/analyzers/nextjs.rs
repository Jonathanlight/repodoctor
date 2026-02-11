use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::analyzers::traits::{Analyzer, AnalyzerCategory, Issue, Severity};
use crate::core::project::Project;
use crate::frameworks::detector::Framework;

pub struct NextJsAnalyzer;

/// Parsed subset of package.json relevant to Next.js checks.
struct PackageJson {
    dependencies: HashMap<String, String>,
    dev_dependencies: HashMap<String, String>,
}

impl PackageJson {
    fn parse(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path.join("package.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;

        let dependencies = Self::parse_dep_map(json.get("dependencies"));
        let dev_dependencies = Self::parse_dep_map(json.get("devDependencies"));

        Some(Self {
            dependencies,
            dev_dependencies,
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

    fn has_dep(&self, name: &str) -> bool {
        self.dependencies.contains_key(name)
    }

    fn has_any_dep(&self, name: &str) -> bool {
        self.dependencies.contains_key(name) || self.dev_dependencies.contains_key(name)
    }

    fn dep_version(&self, name: &str) -> Option<&str> {
        self.dependencies
            .get(name)
            .or_else(|| self.dev_dependencies.get(name))
            .map(|s| s.as_str())
    }
}

/// Try to read next.config.{js,mjs,ts} and return its content.
fn read_next_config(path: &Path) -> Option<(PathBuf, String)> {
    for ext in &["js", "mjs", "ts"] {
        let config_path = path.join(format!("next.config.{}", ext));
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            return Some((config_path, content));
        }
    }
    None
}

/// Directories to skip when walking the project tree.
const SKIP_DIRS: &[&str] = &[".git", "node_modules", ".next", "out", "coverage"];

#[async_trait]
impl Analyzer for NextJsAnalyzer {
    fn name(&self) -> &'static str {
        "nextjs"
    }

    fn description(&self) -> &'static str {
        "Next.js-specific project structure, configuration, and best practices"
    }

    fn category(&self) -> AnalyzerCategory {
        AnalyzerCategory::Structure
    }

    fn applies_to(&self, project: &Project) -> bool {
        project.detected.framework == Framework::NextJs
    }

    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let path = &project.path;
        let pkg = PackageJson::parse(path);
        let next_config = read_next_config(path);

        // Structure checks
        check_app_missing_layout(path, &mut issues);
        check_router_mixing(path, &mut issues);
        check_missing_error_page(path, &mut issues);
        check_missing_app_utilities(path, &mut issues);
        check_missing_robots_txt(path, &mut issues);
        check_missing_sitemap(path, &mut issues);

        // Configuration checks
        check_next_config_empty(&next_config, &mut issues);
        check_tsconfig_strict(path, &mut issues);
        check_next_config_images(&next_config, &mut issues);
        check_next_config_strict_mode(&next_config, &mut issues);
        check_gitignore_env(path, &mut issues);

        // Dependencies checks
        if let Some(ref p) = pkg {
            check_missing_core_deps(p, path, &mut issues);
            check_next_version(p, path, &mut issues);
            check_heavy_bundle_deps(p, path, &mut issues);
        }

        // Testing checks
        check_missing_test_config(path, &mut issues);
        check_missing_test_dirs(path, &mut issues);
        if let Some(ref p) = pkg {
            check_missing_test_library(p, path, &mut issues);
        }

        // Security checks
        check_public_env_secrets(path, &mut issues);
        check_next_config_headers(&next_config, &mut issues);
        check_unsafe_inner_html(path, &mut issues);

        Ok(issues)
    }
}

// ---------------------------------------------------------------------------
// Structure checks
// ---------------------------------------------------------------------------

fn check_app_missing_layout(path: &Path, issues: &mut Vec<Issue>) {
    let app_dir = path.join("app");
    if !app_dir.is_dir() {
        return;
    }

    let has_layout = ["layout.tsx", "layout.jsx", "layout.js"]
        .iter()
        .any(|f| app_dir.join(f).exists());

    if !has_layout {
        issues.push(Issue {
            id: "NJS-001".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::High,
            title: "app/ directory missing layout file".to_string(),
            description: "app/ exists but no layout.tsx/jsx/js found. App Router requires a root layout.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create app/layout.tsx with a root layout component".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_router_mixing(path: &Path, issues: &mut Vec<Issue>) {
    if path.join("app").is_dir() && path.join("pages").is_dir() {
        issues.push(Issue {
            id: "NJS-002".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: "Both app/ and pages/ directories exist".to_string(),
            description: "Mixing App Router and Pages Router can cause routing conflicts.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Migrate fully to App Router (app/) or keep only pages/".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_missing_error_page(path: &Path, issues: &mut Vec<Issue>) {
    let has_app_error = path.join("app").is_dir()
        && ["error.tsx", "error.jsx", "error.js"]
            .iter()
            .any(|f| path.join("app").join(f).exists());

    let has_pages_error = path.join("pages").is_dir()
        && ["_error.tsx", "_error.jsx", "_error.js"]
            .iter()
            .any(|f| path.join("pages").join(f).exists());

    if !has_app_error && !has_pages_error {
        issues.push(Issue {
            id: "NJS-003".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: "Missing error page".to_string(),
            description: "No error.tsx in app/ or _error.tsx in pages/. Custom error pages improve user experience.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create app/error.tsx or pages/_error.tsx for custom error handling".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_missing_app_utilities(path: &Path, issues: &mut Vec<Issue>) {
    let app_dir = path.join("app");
    if !app_dir.is_dir() {
        return;
    }

    let has_not_found = ["not-found.tsx", "not-found.jsx", "not-found.js"]
        .iter()
        .any(|f| app_dir.join(f).exists());

    let has_loading = ["loading.tsx", "loading.jsx", "loading.js"]
        .iter()
        .any(|f| app_dir.join(f).exists());

    if !has_not_found || !has_loading {
        let mut missing = Vec::new();
        if !has_not_found {
            missing.push("not-found.tsx");
        }
        if !has_loading {
            missing.push("loading.tsx");
        }

        issues.push(Issue {
            id: "NJS-004".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Low,
            title: format!("app/ missing: {}", missing.join(", ")),
            description: format!(
                "app/ is missing {}. These improve user experience.",
                missing.join(" and ")
            ),
            file: None,
            line: None,
            suggestion: Some(format!("Create {} in app/", missing.join(" and "))),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_missing_robots_txt(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("public/robots.txt").exists() {
        issues.push(Issue {
            id: "NJS-051".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Low,
            title: "Missing public/robots.txt".to_string(),
            description: "No robots.txt found. Search engines need this for crawling instructions.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create public/robots.txt with appropriate crawling rules".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_missing_sitemap(path: &Path, issues: &mut Vec<Issue>) {
    let has_static_sitemap = path.join("public/sitemap.xml").exists();
    let has_app_sitemap = ["sitemap.ts", "sitemap.js", "sitemap.tsx", "sitemap.jsx"]
        .iter()
        .any(|f| path.join("app").join(f).exists());

    // Check for next-sitemap in package.json
    let has_next_sitemap = PackageJson::parse(path)
        .map(|p| p.has_any_dep("next-sitemap"))
        .unwrap_or(false);

    if !has_static_sitemap && !has_app_sitemap && !has_next_sitemap {
        issues.push(Issue {
            id: "NJS-052".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Info,
            title: "No sitemap configuration found".to_string(),
            description: "No sitemap.xml, app/sitemap.ts, or next-sitemap package found.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Add a sitemap via public/sitemap.xml, app/sitemap.ts, or next-sitemap package".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Configuration checks
// ---------------------------------------------------------------------------

fn check_next_config_empty(
    next_config: &Option<(PathBuf, String)>,
    issues: &mut Vec<Issue>,
) {
    match next_config {
        Some((path, content)) => {
            if content.len() < 10 {
                issues.push(Issue {
                    id: "NJS-010".to_string(),
                    analyzer: "nextjs".to_string(),
                    category: AnalyzerCategory::Configuration,
                    severity: Severity::High,
                    title: "next.config.* is nearly empty".to_string(),
                    description: format!(
                        "{} has less than 10 bytes of content.",
                        path.display()
                    ),
                    file: Some(path.clone()),
                    line: None,
                    suggestion: Some("Add meaningful configuration to next.config".to_string()),
                    auto_fixable: false,
                    references: vec![],
                });
            }
        }
        None => {
            issues.push(Issue {
                id: "NJS-010".to_string(),
                analyzer: "nextjs".to_string(),
                category: AnalyzerCategory::Configuration,
                severity: Severity::High,
                title: "Missing next.config.*".to_string(),
                description: "No next.config.js, next.config.mjs, or next.config.ts found.".to_string(),
                file: None,
                line: None,
                suggestion: Some("Create next.config.js with your project configuration".to_string()),
                auto_fixable: true,
                references: vec![],
            });
        }
    }
}

fn check_tsconfig_strict(path: &Path, issues: &mut Vec<Issue>) {
    let tsconfig_path = path.join("tsconfig.json");
    let content = match std::fs::read_to_string(&tsconfig_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    if !content.contains("\"strict\": true") && !content.contains("\"strict\":true") {
        issues.push(Issue {
            id: "NJS-011".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Medium,
            title: "tsconfig.json missing strict mode".to_string(),
            description: "tsconfig.json exists but \"strict\": true is not set.".to_string(),
            file: Some(tsconfig_path),
            line: None,
            suggestion: Some("Add \"strict\": true to compilerOptions in tsconfig.json".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_next_config_images(
    next_config: &Option<(PathBuf, String)>,
    issues: &mut Vec<Issue>,
) {
    if let Some((path, content)) = next_config {
        if !content.contains("images") {
            issues.push(Issue {
                id: "NJS-012".to_string(),
                analyzer: "nextjs".to_string(),
                category: AnalyzerCategory::Configuration,
                severity: Severity::Low,
                title: "next.config.* missing images config".to_string(),
                description: "next.config does not configure images optimization.".to_string(),
                file: Some(path.clone()),
                line: None,
                suggestion: Some("Add images configuration for optimized image handling".to_string()),
                auto_fixable: false,
                references: vec![],
            });
        }
    }
}

fn check_next_config_strict_mode(
    next_config: &Option<(PathBuf, String)>,
    issues: &mut Vec<Issue>,
) {
    if let Some((path, content)) = next_config {
        if !content.contains("reactStrictMode") {
            issues.push(Issue {
                id: "NJS-013".to_string(),
                analyzer: "nextjs".to_string(),
                category: AnalyzerCategory::Configuration,
                severity: Severity::Medium,
                title: "next.config.* missing reactStrictMode".to_string(),
                description: "reactStrictMode: true is not set in next.config. It helps catch common React bugs.".to_string(),
                file: Some(path.clone()),
                line: None,
                suggestion: Some("Add reactStrictMode: true to next.config".to_string()),
                auto_fixable: true,
                references: vec![],
            });
        }
    }
}

fn check_gitignore_env(path: &Path, issues: &mut Vec<Issue>) {
    let gitignore_path = path.join(".gitignore");
    let content = match std::fs::read_to_string(&gitignore_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let has_env_local = content.lines().any(|l| {
        let t = l.trim();
        t == ".env.local" || t == ".env*.local" || t == ".env.*"
    });

    if !has_env_local {
        issues.push(Issue {
            id: "NJS-050".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Medium,
            title: ".gitignore missing .env.local".to_string(),
            description: ".gitignore should include .env.local or .env*.local to prevent leaking secrets.".to_string(),
            file: Some(gitignore_path),
            line: None,
            suggestion: Some("Add .env*.local to .gitignore".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Dependencies checks
// ---------------------------------------------------------------------------

fn check_missing_core_deps(pkg: &PackageJson, path: &Path, issues: &mut Vec<Issue>) {
    let required = ["next", "react", "react-dom"];
    let missing: Vec<&str> = required
        .iter()
        .filter(|dep| !pkg.has_dep(dep))
        .copied()
        .collect();

    if !missing.is_empty() {
        issues.push(Issue {
            id: "NJS-020".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Dependencies,
            severity: Severity::High,
            title: format!("Missing core dependencies: {}", missing.join(", ")),
            description: format!(
                "package.json is missing {} in dependencies.",
                missing.join(", ")
            ),
            file: Some(path.join("package.json")),
            line: None,
            suggestion: Some(format!("Run `npm install {}`", missing.join(" "))),
            auto_fixable: false,
            references: vec![],
        });
    }
}

/// Parse major version from an npm version constraint string.
fn parse_npm_major_version(constraint: &str) -> Option<u32> {
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

fn check_next_version(pkg: &PackageJson, path: &Path, issues: &mut Vec<Issue>) {
    if let Some(version) = pkg.dep_version("next") {
        if let Some(major) = parse_npm_major_version(version) {
            if major < 14 {
                issues.push(Issue {
                    id: "NJS-021".to_string(),
                    analyzer: "nextjs".to_string(),
                    category: AnalyzerCategory::Dependencies,
                    severity: Severity::High,
                    title: format!("Outdated Next.js version (v{})", major),
                    description: format!(
                        "Next.js version {} is below v14. Consider upgrading for App Router stability and performance.",
                        version
                    ),
                    file: Some(path.join("package.json")),
                    line: None,
                    suggestion: Some("Upgrade to Next.js 14+ for latest features and security fixes".to_string()),
                    auto_fixable: false,
                    references: vec![],
                });
            }
        }
    }
}

fn check_heavy_bundle_deps(pkg: &PackageJson, path: &Path, issues: &mut Vec<Issue>) {
    let heavy = ["moment", "lodash"];
    let found: Vec<&str> = heavy
        .iter()
        .filter(|dep| pkg.has_dep(dep))
        .copied()
        .collect();

    if !found.is_empty() {
        issues.push(Issue {
            id: "NJS-022".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Dependencies,
            severity: Severity::Low,
            title: format!("Heavy bundle dependencies: {}", found.join(", ")),
            description: format!(
                "{} are large packages that increase bundle size. Consider lighter alternatives.",
                found.join(", ")
            ),
            file: Some(path.join("package.json")),
            line: None,
            suggestion: Some("Use date-fns instead of moment, lodash-es or individual lodash imports instead of lodash".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Testing checks
// ---------------------------------------------------------------------------

fn check_missing_test_config(path: &Path, issues: &mut Vec<Issue>) {
    let config_files = [
        "jest.config.js",
        "jest.config.ts",
        "jest.config.mjs",
        "vitest.config.js",
        "vitest.config.ts",
        "vitest.config.mjs",
        "cypress.config.js",
        "cypress.config.ts",
        "cypress.config.mjs",
    ];

    let has_config = config_files.iter().any(|f| path.join(f).exists());

    if !has_config {
        issues.push(Issue {
            id: "NJS-030".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::High,
            title: "No test framework configuration found".to_string(),
            description: "No jest, vitest, or cypress config file found.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Set up a testing framework (Jest, Vitest, or Cypress)".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_missing_test_dirs(path: &Path, issues: &mut Vec<Issue>) {
    let test_dirs = ["__tests__", "tests", "test", "cypress"];
    let has_dir = test_dirs.iter().any(|d| path.join(d).is_dir());

    if !has_dir {
        issues.push(Issue {
            id: "NJS-031".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::Medium,
            title: "No test directory found".to_string(),
            description: "No __tests__/, tests/, test/, or cypress/ directory found.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create a test directory and add automated tests".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_missing_test_library(pkg: &PackageJson, path: &Path, issues: &mut Vec<Issue>) {
    let test_libs = [
        "jest",
        "vitest",
        "@testing-library/react",
        "@testing-library/jest-dom",
        "cypress",
        "playwright",
        "@playwright/test",
    ];

    let has_test_lib = test_libs.iter().any(|lib| pkg.has_any_dep(lib));

    if !has_test_lib {
        issues.push(Issue {
            id: "NJS-032".to_string(),
            analyzer: "nextjs".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::Medium,
            title: "No testing library in dependencies".to_string(),
            description: "No testing library (jest, vitest, testing-library, cypress, playwright) found in package.json.".to_string(),
            file: Some(path.join("package.json")),
            line: None,
            suggestion: Some("Install a testing library: npm install --save-dev jest @testing-library/react".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Security checks
// ---------------------------------------------------------------------------

fn check_public_env_secrets(path: &Path, issues: &mut Vec<Issue>) {
    let sensitive_suffixes = ["SECRET", "PASSWORD", "KEY", "TOKEN"];
    let re = Regex::new(r"process\.env\.NEXT_PUBLIC_(\w+)").unwrap();

    let source_dirs: Vec<PathBuf> = ["app", "pages", "src", "components"]
        .iter()
        .map(|d| path.join(d))
        .filter(|d| d.is_dir())
        .collect();

    for source_dir in &source_dirs {
        for entry in WalkDir::new(source_dir)
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
            if !name.ends_with(".tsx")
                && !name.ends_with(".jsx")
                && !name.ends_with(".ts")
                && !name.ends_with(".js")
            {
                continue;
            }

            let file_path = entry.into_path();
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                for (line_num, line) in content.lines().enumerate() {
                    for cap in re.captures_iter(line) {
                        let env_name = &cap[1];
                        if sensitive_suffixes
                            .iter()
                            .any(|s| env_name.to_uppercase().ends_with(s))
                        {
                            issues.push(Issue {
                                id: "NJS-040".to_string(),
                                analyzer: "nextjs".to_string(),
                                category: AnalyzerCategory::Security,
                                severity: Severity::High,
                                title: format!(
                                    "NEXT_PUBLIC_ env with sensitive suffix: {}",
                                    env_name
                                ),
                                description: format!(
                                    "NEXT_PUBLIC_{} in {} exposes a potentially sensitive value to the client.",
                                    env_name,
                                    file_path.display()
                                ),
                                file: Some(file_path.clone()),
                                line: Some(line_num + 1),
                                suggestion: Some("Remove NEXT_PUBLIC_ prefix for sensitive values; access them server-side only".to_string()),
                                auto_fixable: false,
                                references: vec![],
                            });
                            return; // One finding is enough
                        }
                    }
                }
            }
        }
    }
}

fn check_next_config_headers(
    next_config: &Option<(PathBuf, String)>,
    issues: &mut Vec<Issue>,
) {
    if let Some((path, content)) = next_config {
        if !content.contains("headers") {
            issues.push(Issue {
                id: "NJS-041".to_string(),
                analyzer: "nextjs".to_string(),
                category: AnalyzerCategory::Security,
                severity: Severity::Medium,
                title: "next.config.* missing security headers".to_string(),
                description: "next.config does not define custom headers. Security headers (CSP, HSTS, etc.) are important.".to_string(),
                file: Some(path.clone()),
                line: None,
                suggestion: Some("Add a headers() function to next.config with security headers".to_string()),
                auto_fixable: false,
                references: vec![],
            });
        }
    }
}

/// Detect unsafe innerHTML usage in JSX/TSX files.
// NJS-042: dangerously set inner HTML
fn check_unsafe_inner_html(path: &Path, issues: &mut Vec<Issue>) {
    let pattern = "dangerouslySetInner";

    let source_dirs: Vec<PathBuf> = ["app", "pages", "src", "components"]
        .iter()
        .map(|d| path.join(d))
        .filter(|d| d.is_dir())
        .collect();

    for source_dir in &source_dirs {
        for entry in WalkDir::new(source_dir)
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
            if !name.ends_with(".tsx") && !name.ends_with(".jsx") {
                continue;
            }

            let file_path = entry.into_path();
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                for (line_num, line) in content.lines().enumerate() {
                    if line.contains(pattern) {
                        issues.push(Issue {
                            id: "NJS-042".to_string(),
                            analyzer: "nextjs".to_string(),
                            category: AnalyzerCategory::Security,
                            severity: Severity::High,
                            title: "Unsafe innerHTML usage found".to_string(),
                            description: format!(
                                "Unsafe innerHTML usage in {} can lead to XSS vulnerabilities.",
                                file_path.display()
                            ),
                            file: Some(file_path.clone()),
                            line: Some(line_num + 1),
                            suggestion: Some("Sanitize HTML content or use a safe rendering approach".to_string()),
                            auto_fixable: false,
                            references: vec![],
                        });
                        return; // One finding is enough
                    }
                }
            }
        }
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
                framework: Framework::NextJs,
                language: Language::TypeScript,
                version: None,
                package_manager: Some(PackageManager::Npm),
                has_git: false,
                has_ci: None,
            },
        }
    }

    /// Minimal clean Next.js scaffold.
    fn scaffold_nextjs(tmp: &TempDir) {
        // app/ with layout, error, not-found, loading
        stdfs::create_dir_all(tmp.path().join("app")).unwrap();
        stdfs::write(tmp.path().join("app/layout.tsx"), "export default function RootLayout({ children }) { return <html><body>{children}</body></html>; }\n").unwrap();
        stdfs::write(tmp.path().join("app/error.tsx"), "'use client';\nexport default function Error() { return <div>Error</div>; }\n").unwrap();
        stdfs::write(tmp.path().join("app/not-found.tsx"), "export default function NotFound() { return <div>404</div>; }\n").unwrap();
        stdfs::write(tmp.path().join("app/loading.tsx"), "export default function Loading() { return <div>Loading...</div>; }\n").unwrap();

        // public/ with robots.txt and sitemap
        stdfs::create_dir_all(tmp.path().join("public")).unwrap();
        stdfs::write(tmp.path().join("public/robots.txt"), "User-agent: *\nAllow: /\n").unwrap();
        stdfs::write(tmp.path().join("public/sitemap.xml"), "<urlset></urlset>\n").unwrap();

        // next.config.mjs with all expected sections
        stdfs::write(
            tmp.path().join("next.config.mjs"),
            "/** @type {import('next').NextConfig} */\nconst nextConfig = {\n  reactStrictMode: true,\n  images: { domains: [] },\n  async headers() { return []; },\n};\nexport default nextConfig;\n",
        )
        .unwrap();

        // tsconfig.json with strict
        stdfs::write(
            tmp.path().join("tsconfig.json"),
            "{\n  \"compilerOptions\": {\n    \"strict\": true\n  }\n}\n",
        )
        .unwrap();

        // .gitignore
        stdfs::write(
            tmp.path().join(".gitignore"),
            ".next\nnode_modules\n.env.local\n",
        )
        .unwrap();

        // __tests__/ directory
        stdfs::create_dir_all(tmp.path().join("__tests__")).unwrap();

        // jest config
        stdfs::write(
            tmp.path().join("jest.config.js"),
            "module.exports = { testEnvironment: 'jsdom' };\n",
        )
        .unwrap();

        // package.json with all core deps + test lib
        stdfs::write(
            tmp.path().join("package.json"),
            r#"{
  "dependencies": {
    "next": "^14.0.0",
    "react": "^18.0.0",
    "react-dom": "^18.0.0"
  },
  "devDependencies": {
    "jest": "^29.0.0",
    "@testing-library/react": "^14.0.0"
  }
}"#,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_applies_only_to_nextjs() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);
        assert!(NextJsAnalyzer.applies_to(&project));

        let non_nextjs = Project {
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
        assert!(!NextJsAnalyzer.applies_to(&non_nextjs));
    }

    #[tokio::test]
    async fn test_clean_nextjs_project() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(
            issues.is_empty(),
            "Expected no issues but got: {:?}",
            issues.iter().map(|i| &i.id).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_app_missing_layout() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::remove_file(tmp.path().join("app/layout.tsx")).unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-001"));
    }

    #[tokio::test]
    async fn test_router_mixing() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::create_dir_all(tmp.path().join("pages")).unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-002"));
    }

    #[tokio::test]
    async fn test_missing_error_page() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::remove_file(tmp.path().join("app/error.tsx")).unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-003"));
    }

    #[tokio::test]
    async fn test_missing_app_utilities() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::remove_file(tmp.path().join("app/not-found.tsx")).unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-004"));
    }

    #[tokio::test]
    async fn test_next_config_empty() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::write(tmp.path().join("next.config.mjs"), "{}").unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-010"));
    }

    #[tokio::test]
    async fn test_next_config_missing() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::remove_file(tmp.path().join("next.config.mjs")).unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-010"));
    }

    #[tokio::test]
    async fn test_tsconfig_not_strict() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::write(
            tmp.path().join("tsconfig.json"),
            "{\n  \"compilerOptions\": {}\n}\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-011"));
    }

    #[tokio::test]
    async fn test_missing_core_deps() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::write(
            tmp.path().join("package.json"),
            r#"{"dependencies":{"next":"^14.0.0"},"devDependencies":{"jest":"^29.0.0","@testing-library/react":"^14.0.0"}}"#,
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-020"));
    }

    #[tokio::test]
    async fn test_outdated_next_version() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::write(
            tmp.path().join("package.json"),
            r#"{"dependencies":{"next":"^13.0.0","react":"^18.0.0","react-dom":"^18.0.0"},"devDependencies":{"jest":"^29.0.0","@testing-library/react":"^14.0.0"}}"#,
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-021"));
    }

    #[tokio::test]
    async fn test_heavy_bundle_deps() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::write(
            tmp.path().join("package.json"),
            r#"{"dependencies":{"next":"^14.0.0","react":"^18.0.0","react-dom":"^18.0.0","moment":"^2.29.0","lodash":"^4.17.0"},"devDependencies":{"jest":"^29.0.0","@testing-library/react":"^14.0.0"}}"#,
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-022"));
    }

    #[tokio::test]
    async fn test_missing_test_config() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::remove_file(tmp.path().join("jest.config.js")).unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-030"));
    }

    #[tokio::test]
    async fn test_missing_test_dirs() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::remove_dir_all(tmp.path().join("__tests__")).unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-031"));
    }

    #[tokio::test]
    async fn test_missing_test_library() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::write(
            tmp.path().join("package.json"),
            r#"{"dependencies":{"next":"^14.0.0","react":"^18.0.0","react-dom":"^18.0.0"},"devDependencies":{}}"#,
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-032"));
    }

    #[tokio::test]
    async fn test_public_env_secrets() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::write(
            tmp.path().join("app/page.tsx"),
            "const key = process.env.NEXT_PUBLIC_API_SECRET;\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-040"));
    }

    #[tokio::test]
    async fn test_unsafe_inner_html() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        // Write a JSX file with the unsafe pattern
        let unsafe_html = "dangerously";
        let set_inner = "SetInnerHTML";
        stdfs::write(
            tmp.path().join("app/page.tsx"),
            format!("<div {}{}={{{{ __html: content }}}} />\n", unsafe_html, set_inner),
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-042"));
    }

    #[tokio::test]
    async fn test_gitignore_missing_env_local() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::write(tmp.path().join(".gitignore"), "node_modules\n").unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-050"));
    }

    #[tokio::test]
    async fn test_missing_robots_txt() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::remove_file(tmp.path().join("public/robots.txt")).unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-051"));
    }

    #[tokio::test]
    async fn test_missing_sitemap() {
        let tmp = TempDir::new().unwrap();
        scaffold_nextjs(&tmp);
        stdfs::remove_file(tmp.path().join("public/sitemap.xml")).unwrap();
        let project = make_project(&tmp);
        let issues = NextJsAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "NJS-052"));
    }

    #[tokio::test]
    async fn test_parse_npm_major_version() {
        assert_eq!(parse_npm_major_version("^14.0.0"), Some(14));
        assert_eq!(parse_npm_major_version("~13.4.0"), Some(13));
        assert_eq!(parse_npm_major_version(">=12.0.0"), Some(12));
        assert_eq!(parse_npm_major_version("14.0.0"), Some(14));
        assert_eq!(parse_npm_major_version("invalid"), None);
    }
}
