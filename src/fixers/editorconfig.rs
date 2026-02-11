use anyhow::Result;
use std::fs;

use crate::analyzers::traits::Issue;
use crate::core::project::Project;

use super::traits::{FixResult, Fixer};

pub struct EditorConfigFixer;

const EDITORCONFIG_TEMPLATE: &str = "root = true

[*]
indent_style = space
indent_size = 4
end_of_line = lf
charset = utf-8
trim_trailing_whitespace = true
insert_final_newline = true
";

impl Fixer for EditorConfigFixer {
    fn handles(&self) -> &[&str] {
        &["CFG-002"]
    }

    fn describe(&self, _issue: &Issue, _project: &Project) -> String {
        "Create .editorconfig with standard settings".to_string()
    }

    fn apply(&self, _issue: &Issue, project: &Project) -> Result<FixResult> {
        let path = project.path.join(".editorconfig");
        if path.exists() {
            return Ok(FixResult::Skipped {
                reason: ".editorconfig already exists".to_string(),
            });
        }
        fs::write(&path, EDITORCONFIG_TEMPLATE)?;
        Ok(FixResult::Applied {
            description: "Created .editorconfig".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::traits::{AnalyzerCategory, Severity};
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

    fn make_issue() -> Issue {
        Issue {
            id: "CFG-002".to_string(),
            analyzer: "config_files".to_string(),
            category: AnalyzerCategory::Configuration,
            severity: Severity::Low,
            title: "Missing .editorconfig".to_string(),
            description: String::new(),
            file: None,
            line: None,
            suggestion: None,
            auto_fixable: true,
            references: vec![],
        }
    }

    #[test]
    fn test_creates_editorconfig() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);
        let issue = make_issue();

        let fixer = EditorConfigFixer;
        let result = fixer.apply(&issue, &project).unwrap();

        assert!(matches!(result, FixResult::Applied { .. }));
        let content = stdfs::read_to_string(tmp.path().join(".editorconfig")).unwrap();
        assert!(content.contains("root = true"));
        assert!(content.contains("indent_style = space"));
        assert!(content.contains("indent_size = 4"));
    }

    #[test]
    fn test_skips_existing_editorconfig() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join(".editorconfig"), "root = true\n").unwrap();
        let project = make_project(&tmp);
        let issue = make_issue();

        let fixer = EditorConfigFixer;
        let result = fixer.apply(&issue, &project).unwrap();

        assert!(matches!(result, FixResult::Skipped { .. }));
    }
}
