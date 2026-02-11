use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use walkdir::WalkDir;

use crate::analyzers::traits::{Analyzer, AnalyzerCategory, Issue, Severity};
use crate::core::project::Project;
use crate::frameworks::detector::Framework;

pub struct FlutterAnalyzer;

/// Parsed subset of pubspec.yaml relevant to Flutter checks.
struct PubspecYaml {
    description: Option<String>,
    sdk_constraint: Option<String>,
    dependencies: Vec<String>,
    dev_dependencies: Vec<String>,
    /// Dependencies using git source.
    git_deps: Vec<String>,
}

impl PubspecYaml {
    fn parse(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path.join("pubspec.yaml")).ok()?;
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;

        let description = yaml
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let sdk_constraint = yaml
            .get("environment")
            .and_then(|e| e.get("sdk"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let dependencies = Self::extract_dep_names(yaml.get("dependencies"));
        let dev_dependencies = Self::extract_dep_names(yaml.get("dev_dependencies"));

        let git_deps = Self::extract_git_deps(yaml.get("dependencies"));

        Some(Self {
            description,
            sdk_constraint,
            dependencies,
            dev_dependencies,
            git_deps,
        })
    }

    fn extract_dep_names(value: Option<&serde_yaml::Value>) -> Vec<String> {
        value
            .and_then(|v| v.as_mapping())
            .map(|m| {
                m.keys()
                    .filter_map(|k| k.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn extract_git_deps(value: Option<&serde_yaml::Value>) -> Vec<String> {
        let mapping = match value.and_then(|v| v.as_mapping()) {
            Some(m) => m,
            None => return Vec::new(),
        };

        let mut result = Vec::new();
        for (key, val) in mapping {
            if let Some(name) = key.as_str() {
                if val.is_mapping() && val.get("git").is_some() {
                    result.push(name.to_string());
                }
            }
        }
        result
    }

    fn has_dep(&self, name: &str) -> bool {
        self.dependencies.iter().any(|d| d == name)
    }

    fn has_dev_dep(&self, name: &str) -> bool {
        self.dev_dependencies.iter().any(|d| d == name)
    }

    fn has_any_dep(&self, name: &str) -> bool {
        self.has_dep(name) || self.has_dev_dep(name)
    }
}

/// Directories to skip when walking the project tree.
const SKIP_DIRS: &[&str] = &[".git", ".dart_tool", "build", ".pub-cache", "node_modules"];

#[async_trait]
impl Analyzer for FlutterAnalyzer {
    fn name(&self) -> &'static str {
        "flutter"
    }

    fn description(&self) -> &'static str {
        "Flutter-specific project structure, configuration, and best practices"
    }

    fn category(&self) -> AnalyzerCategory {
        AnalyzerCategory::Structure
    }

    fn applies_to(&self, project: &Project) -> bool {
        project.detected.framework == Framework::Flutter
    }

    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let path = &project.path;
        let pubspec = PubspecYaml::parse(path);

        // Structure checks
        check_main_dart_too_large(path, &mut issues);
        check_no_architecture(path, &mut issues);
        check_missing_platform_icons(path, &mut issues);
        check_gitignore_entries(path, &mut issues);

        // Configuration checks
        if let Some(ref p) = pubspec {
            check_missing_description(p, path, &mut issues);
            check_sdk_constraint(p, path, &mut issues);
        }
        check_android_signing(path, &mut issues);
        check_ios_info_plist(path, &mut issues);

        // Dependencies checks
        if let Some(ref p) = pubspec {
            check_dev_deps_in_dependencies(p, path, &mut issues);
            check_git_dependencies(p, path, &mut issues);
        }

        // Testing checks
        check_no_widget_tests(path, &mut issues);
        check_missing_integration_tests(path, &mut issues);
        if let Some(ref p) = pubspec {
            check_missing_flutter_test(p, path, &mut issues);
        }

        // Security checks
        check_http_urls(path, &mut issues);
        check_debug_prints(path, &mut issues);

        Ok(issues)
    }
}

// ---------------------------------------------------------------------------
// Structure checks
// ---------------------------------------------------------------------------

fn check_main_dart_too_large(path: &Path, issues: &mut Vec<Issue>) {
    let main_dart = path.join("lib/main.dart");
    let content = match std::fs::read_to_string(&main_dart) {
        Ok(c) => c,
        Err(_) => return,
    };

    let non_blank = content.lines().filter(|l| !l.trim().is_empty()).count();
    if non_blank > 50 {
        issues.push(Issue {
            id: "FLT-003".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: "lib/main.dart is too large".to_string(),
            description: format!(
                "lib/main.dart has {} non-blank lines. Business logic should be separated into dedicated files.",
                non_blank
            ),
            file: Some(main_dart),
            line: None,
            suggestion: Some("Extract widgets and business logic into separate files under lib/".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_no_architecture(path: &Path, issues: &mut Vec<Issue>) {
    let lib_dir = path.join("lib");
    if !lib_dir.is_dir() {
        return;
    }

    let has_subdirs = std::fs::read_dir(&lib_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        })
        .unwrap_or(false);

    if has_subdirs {
        return;
    }

    let dart_file_count = std::fs::read_dir(&lib_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().map(|t| t.is_file()).unwrap_or(false)
                        && e.file_name().to_string_lossy().ends_with(".dart")
                })
                .count()
        })
        .unwrap_or(0);

    if dart_file_count > 3 {
        issues.push(Issue {
            id: "FLT-004".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: "No architecture structure in lib/".to_string(),
            description: format!(
                "Found {} .dart files flat in lib/ with no subdirectories. Consider organizing code into folders.",
                dart_file_count
            ),
            file: None,
            line: None,
            suggestion: Some("Create subdirectories like lib/screens/, lib/widgets/, lib/models/".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_missing_platform_icons(path: &Path, issues: &mut Vec<Issue>) {
    let checks = [
        ("android", "android/app/src/main/res/mipmap-hdpi"),
        ("ios", "ios/Runner/Assets.xcassets/AppIcon.appiconset"),
    ];

    for (platform, icon_path) in &checks {
        let platform_dir = path.join(platform);
        if platform_dir.is_dir() && !path.join(icon_path).is_dir() {
            issues.push(Issue {
                id: "FLT-052".to_string(),
                analyzer: "flutter".to_string(),
                category: AnalyzerCategory::Structure,
                severity: Severity::Low,
                title: format!("Missing {} icon assets", platform),
                description: format!(
                    "{} platform directory exists but icon assets at {} are missing.",
                    platform, icon_path
                ),
                file: None,
                line: None,
                suggestion: Some(format!("Add proper icon assets for {} platform", platform)),
                auto_fixable: false,
                references: vec![],
            });
        }
    }
}

fn check_gitignore_entries(path: &Path, issues: &mut Vec<Issue>) {
    let gitignore_path = path.join(".gitignore");
    let content = match std::fs::read_to_string(&gitignore_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let required = ["build/", ".dart_tool/", ".flutter-plugins"];
    let mut missing = Vec::new();

    for entry in &required {
        let base = entry.trim_end_matches('/');
        let found = content.lines().any(|l| {
            let t = l.trim();
            t == *entry || t == format!("/{}", entry) || t == base
        });
        if !found {
            missing.push(*entry);
        }
    }

    if !missing.is_empty() {
        issues.push(Issue {
            id: "FLT-053".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::Medium,
            title: format!(".gitignore missing: {}", missing.join(", ")),
            description: format!(
                ".gitignore should include {} for Flutter projects.",
                missing.join(", ")
            ),
            file: Some(gitignore_path),
            line: None,
            suggestion: Some(format!("Add {} to .gitignore", missing.join(", "))),
            auto_fixable: true,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Configuration checks
// ---------------------------------------------------------------------------

fn check_missing_description(pubspec: &PubspecYaml, path: &Path, issues: &mut Vec<Issue>) {
    let is_missing = pubspec
        .description
        .as_ref()
        .map(|d| d.trim().is_empty())
        .unwrap_or(true);

    if is_missing {
        issues.push(Issue {
            id: "FLT-010".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Low,
            title: "Missing description in pubspec.yaml".to_string(),
            description: "pubspec.yaml is missing a description field.".to_string(),
            file: Some(path.join("pubspec.yaml")),
            line: None,
            suggestion: Some("Add a meaningful description field to pubspec.yaml".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_sdk_constraint(pubspec: &PubspecYaml, path: &Path, issues: &mut Vec<Issue>) {
    let constraint = match &pubspec.sdk_constraint {
        Some(c) => c,
        None => return,
    };

    // Extract the minimum version from constraints like ">=2.19.0 <4.0.0" or "^3.0.0"
    let version_str = constraint
        .trim()
        .trim_start_matches(">=")
        .trim_start_matches('^');

    let major: u32 = match version_str.split('.').next().and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return,
    };

    if major < 3 {
        issues.push(Issue {
            id: "FLT-011".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::High,
            title: "SDK constraint below Dart 3.0".to_string(),
            description: format!(
                "environment.sdk is '{}'. Dart 3+ brings sound null safety and modern features.",
                constraint
            ),
            file: Some(path.join("pubspec.yaml")),
            line: None,
            suggestion: Some("Update SDK constraint to '^3.0.0' or higher".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_android_signing(path: &Path, issues: &mut Vec<Issue>) {
    let gradle_path = path.join("android/app/build.gradle");
    let content = match std::fs::read_to_string(&gradle_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    if !content.contains("signingConfigs") {
        issues.push(Issue {
            id: "FLT-050".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Medium,
            title: "Android build.gradle missing signingConfigs".to_string(),
            description: "android/app/build.gradle exists but has no signingConfigs for release builds.".to_string(),
            file: Some(gradle_path),
            line: None,
            suggestion: Some("Add signingConfigs for release builds in build.gradle".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_ios_info_plist(path: &Path, issues: &mut Vec<Issue>) {
    if path.join("ios").is_dir() && !path.join("ios/Runner/Info.plist").exists() {
        issues.push(Issue {
            id: "FLT-051".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Medium,
            title: "Missing ios/Runner/Info.plist".to_string(),
            description: "ios/ directory exists but ios/Runner/Info.plist is missing.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Run `flutter create .` to regenerate iOS platform files".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Dependencies checks
// ---------------------------------------------------------------------------

fn check_dev_deps_in_dependencies(pubspec: &PubspecYaml, path: &Path, issues: &mut Vec<Issue>) {
    let dev_only_pkgs = [
        "flutter_test",
        "build_runner",
        "mockito",
        "flutter_lints",
        "test",
        "integration_test",
        "fake_async",
    ];

    let misplaced: Vec<&str> = dev_only_pkgs
        .iter()
        .filter(|pkg| pubspec.has_dep(pkg))
        .copied()
        .collect();

    if !misplaced.is_empty() {
        issues.push(Issue {
            id: "FLT-021".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Dependencies,
            severity: Severity::Medium,
            title: format!("Dev-only packages in dependencies: {}", misplaced.join(", ")),
            description: format!(
                "The following packages should be in dev_dependencies: {}",
                misplaced.join(", ")
            ),
            file: Some(path.join("pubspec.yaml")),
            line: None,
            suggestion: Some("Move these packages to dev_dependencies in pubspec.yaml".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_git_dependencies(pubspec: &PubspecYaml, path: &Path, issues: &mut Vec<Issue>) {
    if !pubspec.git_deps.is_empty() {
        issues.push(Issue {
            id: "FLT-022".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Dependencies,
            severity: Severity::Low,
            title: format!("Git dependencies found: {}", pubspec.git_deps.join(", ")),
            description: "Dependencies using git: source can be unstable and hard to reproduce.".to_string(),
            file: Some(path.join("pubspec.yaml")),
            line: None,
            suggestion: Some("Consider publishing packages to pub.dev or using path dependencies".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Testing checks
// ---------------------------------------------------------------------------

fn check_no_widget_tests(path: &Path, issues: &mut Vec<Issue>) {
    let test_dir = path.join("test");
    if !test_dir.is_dir() {
        return;
    }

    let has_widget_test = WalkDir::new(&test_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.file_name().to_string_lossy().ends_with(".dart")
        })
        .any(|e| {
            std::fs::read_to_string(e.path())
                .map(|c| c.contains("testWidgets"))
                .unwrap_or(false)
        });

    if !has_widget_test {
        issues.push(Issue {
            id: "FLT-030".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::High,
            title: "No widget tests found".to_string(),
            description: "test/ directory exists but no file contains testWidgets calls.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Add widget tests using testWidgets() for UI components".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

fn check_missing_integration_tests(path: &Path, issues: &mut Vec<Issue>) {
    if !path.join("integration_test").is_dir() {
        issues.push(Issue {
            id: "FLT-031".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::Medium,
            title: "Missing integration_test/ directory".to_string(),
            description: "No integration_test/ directory found. Integration tests verify complete app flows.".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create integration_test/ and add integration tests".to_string()),
            auto_fixable: true,
            references: vec![],
        });
    }
}

fn check_missing_flutter_test(pubspec: &PubspecYaml, path: &Path, issues: &mut Vec<Issue>) {
    if !pubspec.has_any_dep("flutter_test") {
        issues.push(Issue {
            id: "FLT-032".to_string(),
            analyzer: "flutter".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::High,
            title: "Missing flutter_test dependency".to_string(),
            description: "flutter_test is not in dependencies or dev_dependencies.".to_string(),
            file: Some(path.join("pubspec.yaml")),
            line: None,
            suggestion: Some("Add flutter_test to dev_dependencies in pubspec.yaml".to_string()),
            auto_fixable: false,
            references: vec![],
        });
    }
}

// ---------------------------------------------------------------------------
// Security checks
// ---------------------------------------------------------------------------

/// Check if an http:// URL is a safe local address.
fn is_local_http(line: &str, pos: usize) -> bool {
    let after = &line[pos + 7..]; // skip "http://"
    after.starts_with("localhost")
        || after.starts_with("127.0.0.1")
        || after.starts_with("10.")
}

fn check_http_urls(path: &Path, issues: &mut Vec<Issue>) {
    let lib_dir = path.join("lib");
    if !lib_dir.is_dir() {
        return;
    }

    for entry in WalkDir::new(&lib_dir)
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
        if !entry.file_name().to_string_lossy().ends_with(".dart") {
            continue;
        }

        let file_path = entry.into_path();
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                if let Some(pos) = line.find("http://") {
                    if !is_local_http(line, pos) {
                        issues.push(Issue {
                            id: "FLT-041".to_string(),
                            analyzer: "flutter".to_string(),
                            category: AnalyzerCategory::Security,
                            severity: Severity::High,
                            title: "Insecure HTTP URL found".to_string(),
                            description: format!(
                                "http:// URL found in {}. Use https:// for secure communication.",
                                file_path.display()
                            ),
                            file: Some(file_path.clone()),
                            line: Some(line_num + 1),
                            suggestion: Some("Replace http:// with https://".to_string()),
                            auto_fixable: true,
                            references: vec![],
                        });
                        break; // One issue per file
                    }
                }
            }
        }
    }
}

fn check_debug_prints(path: &Path, issues: &mut Vec<Issue>) {
    let lib_dir = path.join("lib");
    if !lib_dir.is_dir() {
        return;
    }

    for entry in WalkDir::new(&lib_dir)
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
        if !entry.file_name().to_string_lossy().ends_with(".dart") {
            continue;
        }

        let file_path = entry.into_path();
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            for (line_num, line) in content.lines().enumerate() {
                if line.contains("debugPrint(") {
                    issues.push(Issue {
                        id: "FLT-042".to_string(),
                        analyzer: "flutter".to_string(),
                        category: AnalyzerCategory::Security,
                        severity: Severity::High,
                        title: "debugPrint() found in lib/ code".to_string(),
                        description: format!(
                            "debugPrint() call found in {}. Debug output should not be in production code.",
                            file_path.display()
                        ),
                        file: Some(file_path.clone()),
                        line: Some(line_num + 1),
                        suggestion: Some("Remove debugPrint() calls or use a proper logging framework".to_string()),
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
                framework: Framework::Flutter,
                language: Language::Dart,
                version: None,
                package_manager: Some(PackageManager::Pub),
                has_git: false,
                has_ci: None,
            },
        }
    }

    /// Minimal clean Flutter scaffold.
    fn scaffold_flutter(tmp: &TempDir) {
        // lib/ with subdirectories and small main.dart
        stdfs::create_dir_all(tmp.path().join("lib/screens")).unwrap();
        stdfs::write(
            tmp.path().join("lib/main.dart"),
            "import 'package:flutter/material.dart';\nvoid main() => runApp(MyApp());\n",
        )
        .unwrap();

        // test/ with widget tests
        stdfs::create_dir_all(tmp.path().join("test")).unwrap();
        stdfs::write(
            tmp.path().join("test/widget_test.dart"),
            "import 'package:flutter_test/flutter_test.dart';\nvoid main() { testWidgets('pumps', (tester) async {}); }\n",
        )
        .unwrap();

        // integration_test/
        stdfs::create_dir_all(tmp.path().join("integration_test")).unwrap();

        // android with signing + icons
        stdfs::create_dir_all(tmp.path().join("android/app/src/main/res/mipmap-hdpi")).unwrap();
        stdfs::write(
            tmp.path().join("android/app/build.gradle"),
            "android {\n  signingConfigs { release { } }\n}\n",
        )
        .unwrap();

        // ios with Info.plist + icons
        stdfs::create_dir_all(tmp.path().join("ios/Runner/Assets.xcassets/AppIcon.appiconset")).unwrap();
        stdfs::write(
            tmp.path().join("ios/Runner/Info.plist"),
            "<plist></plist>",
        )
        .unwrap();

        // .gitignore
        stdfs::write(
            tmp.path().join(".gitignore"),
            "build/\n.dart_tool/\n.flutter-plugins\n",
        )
        .unwrap();

        // pubspec.yaml — clean
        stdfs::write(
            tmp.path().join("pubspec.yaml"),
            r#"name: my_app
description: A sample Flutter app
environment:
  sdk: ">=3.0.0 <4.0.0"
dependencies:
  flutter:
    sdk: flutter
dev_dependencies:
  flutter_test:
    sdk: flutter
"#,
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_applies_only_to_flutter() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);
        assert!(FlutterAnalyzer.applies_to(&project));

        let non_flutter = Project {
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
        assert!(!FlutterAnalyzer.applies_to(&non_flutter));
    }

    #[tokio::test]
    async fn test_clean_flutter_project() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(
            issues.is_empty(),
            "Expected no issues but got: {:?}",
            issues.iter().map(|i| &i.id).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_main_dart_too_large() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        // Overwrite main.dart with 60 non-blank lines
        let lines: String = (0..60).map(|i| format!("var x{} = {};\n", i, i)).collect();
        stdfs::write(tmp.path().join("lib/main.dart"), lines).unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-003"));
    }

    #[tokio::test]
    async fn test_no_architecture() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        // Remove subdirectory and add flat files
        stdfs::remove_dir_all(tmp.path().join("lib/screens")).unwrap();
        for i in 0..5 {
            stdfs::write(
                tmp.path().join(format!("lib/file{}.dart", i)),
                format!("class File{} {{}}", i),
            )
            .unwrap();
        }
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-004"));
    }

    #[tokio::test]
    async fn test_missing_description() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::write(
            tmp.path().join("pubspec.yaml"),
            "name: my_app\nenvironment:\n  sdk: \">=3.0.0 <4.0.0\"\ndev_dependencies:\n  flutter_test:\n    sdk: flutter\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-010"));
    }

    #[tokio::test]
    async fn test_sdk_below_3() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::write(
            tmp.path().join("pubspec.yaml"),
            "name: my_app\ndescription: test\nenvironment:\n  sdk: \">=2.19.0 <3.0.0\"\ndev_dependencies:\n  flutter_test:\n    sdk: flutter\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-011"));
    }

    #[tokio::test]
    async fn test_dev_deps_in_dependencies() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::write(
            tmp.path().join("pubspec.yaml"),
            r#"name: my_app
description: test
environment:
  sdk: ">=3.0.0 <4.0.0"
dependencies:
  flutter:
    sdk: flutter
  build_runner: ^2.0.0
  mockito: ^5.0.0
dev_dependencies:
  flutter_test:
    sdk: flutter
"#,
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-021"));
    }

    #[tokio::test]
    async fn test_git_dependencies() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::write(
            tmp.path().join("pubspec.yaml"),
            r#"name: my_app
description: test
environment:
  sdk: ">=3.0.0 <4.0.0"
dependencies:
  flutter:
    sdk: flutter
  my_pkg:
    git:
      url: https://github.com/example/my_pkg.git
dev_dependencies:
  flutter_test:
    sdk: flutter
"#,
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-022"));
    }

    #[tokio::test]
    async fn test_no_widget_tests() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        // Overwrite test file without testWidgets
        stdfs::write(
            tmp.path().join("test/widget_test.dart"),
            "void main() { test('unit', () {}); }\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-030"));
    }

    #[tokio::test]
    async fn test_missing_integration_tests() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::remove_dir_all(tmp.path().join("integration_test")).unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-031"));
    }

    #[tokio::test]
    async fn test_missing_flutter_test_dep() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::write(
            tmp.path().join("pubspec.yaml"),
            "name: my_app\ndescription: test\nenvironment:\n  sdk: \">=3.0.0 <4.0.0\"\ndependencies:\n  flutter:\n    sdk: flutter\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-032"));
    }

    #[tokio::test]
    async fn test_http_urls_in_lib() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::write(
            tmp.path().join("lib/api.dart"),
            "final url = 'http://example.com/api';\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-041"));
    }

    #[tokio::test]
    async fn test_http_localhost_allowed() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::write(
            tmp.path().join("lib/api.dart"),
            "final url = 'http://localhost:8080/api';\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(!issues.iter().any(|i| i.id == "FLT-041"));
    }

    #[tokio::test]
    async fn test_debug_print_in_lib() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::write(
            tmp.path().join("lib/utils.dart"),
            "void log() { debugPrint('hello'); }\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-042"));
    }

    #[tokio::test]
    async fn test_android_missing_signing() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::write(
            tmp.path().join("android/app/build.gradle"),
            "android { defaultConfig { } }\n",
        )
        .unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-050"));
    }

    #[tokio::test]
    async fn test_ios_missing_info_plist() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::remove_file(tmp.path().join("ios/Runner/Info.plist")).unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-051"));
    }

    #[tokio::test]
    async fn test_gitignore_missing_entries() {
        let tmp = TempDir::new().unwrap();
        scaffold_flutter(&tmp);
        stdfs::write(tmp.path().join(".gitignore"), ".env\n").unwrap();
        let project = make_project(&tmp);
        let issues = FlutterAnalyzer.analyze(&project).await.unwrap();
        assert!(issues.iter().any(|i| i.id == "FLT-053"));
    }
}
