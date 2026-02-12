use crate::analyzers::traits::Issue;
use crate::core::project::Project;

use super::traits::{Fixer, FixResult};

pub struct FixerRegistry {
    fixers: Vec<Box<dyn Fixer>>,
}

impl FixerRegistry {
    pub fn new(fixers: Vec<Box<dyn Fixer>>) -> Self {
        Self { fixers }
    }

    pub fn find_fixer(&self, issue_id: &str) -> Option<&dyn Fixer> {
        self.fixers
            .iter()
            .find(|f| f.handles().contains(&issue_id))
            .map(|f| f.as_ref())
    }

    pub fn apply_fixes(
        &self,
        issues: &[&Issue],
        project: &Project,
        dry_run: bool,
    ) -> Vec<(String, FixOutcome)> {
        let mut results = Vec::new();

        for issue in issues {
            let outcome = match self.find_fixer(&issue.id) {
                Some(fixer) => {
                    if dry_run {
                        let desc = fixer.describe(issue, project);
                        FixOutcome::DryRun(desc)
                    } else {
                        match fixer.apply(issue, project) {
                            Ok(FixResult::Applied { description }) => {
                                FixOutcome::Applied(description)
                            }
                            Ok(FixResult::Skipped { reason }) => FixOutcome::Skipped(reason),
                            Err(e) => FixOutcome::Error(e.to_string()),
                        }
                    }
                }
                None => FixOutcome::Skipped("No fixer available".to_string()),
            };
            results.push((issue.id.clone(), outcome));
        }

        results
    }
}

pub enum FixOutcome {
    Applied(String),
    Skipped(String),
    DryRun(String),
    Error(String),
}

pub fn default_registry() -> FixerRegistry {
    let fixers: Vec<Box<dyn Fixer>> = vec![
        Box::new(super::directory::DirectoryFixer),
        Box::new(super::gitignore::GitignoreFixer),
        Box::new(super::editorconfig::EditorConfigFixer),
    ];
    FixerRegistry::new(fixers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::traits::{AnalyzerCategory, Issue, Severity};
    use crate::frameworks::detector::{DetectedProject, Framework, Language};
    use std::fs as stdfs;
    use tempfile::TempDir;

    fn make_project(tmp: &TempDir, framework: Framework) -> Project {
        let language = match &framework {
            Framework::RustCargo => Language::Rust,
            Framework::Symfony => Language::Php,
            Framework::Flutter => Language::Dart,
            Framework::NextJs => Language::JavaScript,
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
    fn test_registry_finds_correct_fixer() {
        let registry = default_registry();
        assert!(registry.find_fixer("STR-001").is_some());
        assert!(registry.find_fixer("STR-003").is_some());
        assert!(registry.find_fixer("CFG-002").is_some());
        assert!(registry.find_fixer("UNKNOWN-999").is_none());
    }

    #[test]
    fn test_dry_run_does_not_modify_files() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let issue = make_issue("STR-001", "Missing required directory: src");
        let issues: Vec<&Issue> = vec![&issue];

        let registry = default_registry();
        let results = registry.apply_fixes(&issues, &project, true);

        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].1, FixOutcome::DryRun(_)));
        // Directory should NOT have been created
        assert!(!tmp.path().join("src").exists());
    }

    #[test]
    fn test_apply_fixes_creates_directory() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let issue = make_issue("STR-001", "Missing required directory: src");
        let issues: Vec<&Issue> = vec![&issue];

        let registry = default_registry();
        let results = registry.apply_fixes(&issues, &project, false);

        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].1, FixOutcome::Applied(_)));
        assert!(tmp.path().join("src").exists());
    }

    #[test]
    fn test_apply_fixes_creates_gitignore() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let issue = make_issue("STR-003", "Missing .gitignore");
        let issues: Vec<&Issue> = vec![&issue];

        let registry = default_registry();
        let results = registry.apply_fixes(&issues, &project, false);

        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].1, FixOutcome::Applied(_)));
        let content = stdfs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("target/"));
    }
}
