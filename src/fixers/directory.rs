use anyhow::Result;
use std::fs;

use crate::analyzers::traits::Issue;
use crate::core::project::Project;

use super::traits::{FixResult, Fixer};

pub struct DirectoryFixer;

impl DirectoryFixer {
    fn directory_for_issue(issue: &Issue) -> Option<String> {
        match issue.id.as_str() {
            "STR-001" => {
                // Parse from title: "Missing required directory: {dir}"
                issue
                    .title
                    .strip_prefix("Missing required directory: ")
                    .map(|s| s.to_string())
            }
            "SYM-001" => Some("src/Controller".to_string()),
            "SYM-002" => Some("src/Entity".to_string()),
            "SYM-031" => Some("tests".to_string()),
            "FLT-031" => Some("integration_test".to_string()),
            "NJS-031" => Some("__tests__".to_string()),
            _ => None,
        }
    }
}

impl Fixer for DirectoryFixer {
    fn handles(&self) -> &[&str] {
        &[
            "STR-001", "SYM-001", "SYM-002", "SYM-031", "FLT-031", "NJS-031",
        ]
    }

    fn describe(&self, issue: &Issue, project: &Project) -> String {
        if let Some(dir) = Self::directory_for_issue(issue) {
            format!("Create directory: {}/{}", project.path.display(), dir)
        } else {
            "Create missing directory".to_string()
        }
    }

    fn apply(&self, issue: &Issue, project: &Project) -> Result<FixResult> {
        let dir = match Self::directory_for_issue(issue) {
            Some(d) => d,
            None => {
                return Ok(FixResult::Skipped {
                    reason: "Cannot determine directory to create".to_string(),
                })
            }
        };

        let full_path = project.path.join(&dir);
        if full_path.exists() {
            return Ok(FixResult::Skipped {
                reason: format!("{} already exists", dir),
            });
        }

        fs::create_dir_all(&full_path)?;
        Ok(FixResult::Applied {
            description: format!("Created directory: {}", dir),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::traits::{AnalyzerCategory, Severity};
    use crate::frameworks::detector::{DetectedProject, Framework, Language};
    use tempfile::TempDir;

    fn make_project(tmp: &TempDir, framework: Framework) -> Project {
        Project {
            path: tmp.path().to_path_buf(),
            detected: DetectedProject {
                framework,
                language: Language::Unknown,
                version: None,
                package_manager: None,
                has_git: false,
                has_ci: None,
            },
        }
    }

    fn make_issue(id: &str, title: &str) -> Issue {
        Issue {
            id: id.to_string(),
            analyzer: "test".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::High,
            title: title.to_string(),
            description: String::new(),
            file: None,
            line: None,
            suggestion: None,
            auto_fixable: true,
            references: vec![],
        }
    }

    #[test]
    fn test_creates_missing_directory() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let issue = make_issue("STR-001", "Missing required directory: src");

        let fixer = DirectoryFixer;
        let result = fixer.apply(&issue, &project).unwrap();

        assert!(matches!(result, FixResult::Applied { .. }));
        assert!(tmp.path().join("src").exists());
    }

    #[test]
    fn test_skips_existing_directory() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("src")).unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let issue = make_issue("STR-001", "Missing required directory: src");

        let fixer = DirectoryFixer;
        let result = fixer.apply(&issue, &project).unwrap();

        assert!(matches!(result, FixResult::Skipped { .. }));
    }

    #[test]
    fn test_creates_symfony_controller_dir() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let project = make_project(&tmp, Framework::Symfony);
        let issue = make_issue("SYM-001", "Missing src/Controller/ directory");

        let fixer = DirectoryFixer;
        let result = fixer.apply(&issue, &project).unwrap();

        assert!(matches!(result, FixResult::Applied { .. }));
        assert!(tmp.path().join("src/Controller").exists());
    }

    #[test]
    fn test_creates_flutter_integration_test_dir() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Flutter);
        let issue = make_issue("FLT-031", "Missing integration_test/ directory");

        let fixer = DirectoryFixer;
        let result = fixer.apply(&issue, &project).unwrap();

        assert!(matches!(result, FixResult::Applied { .. }));
        assert!(tmp.path().join("integration_test").exists());
    }
}
