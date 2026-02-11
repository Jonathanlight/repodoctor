use anyhow::Result;
use std::fs;

use crate::analyzers::traits::Issue;
use crate::core::project::Project;
use crate::frameworks::detector::Framework;

use super::traits::{FixResult, Fixer};

pub struct GitignoreFixer;

impl GitignoreFixer {
    fn gitignore_template(framework: &Framework) -> &'static str {
        match framework {
            Framework::Symfony => "vendor/\nvar/\n.env\n.env.local\n",
            Framework::Flutter => {
                "build/\n.dart_tool/\n.flutter-plugins\n.flutter-plugins-dependencies\n"
            }
            Framework::NextJs => ".next/\nnode_modules/\n.env.local\n.env*.local\n",
            Framework::RustCargo => "target/\n",
            _ => ".env\n*.log\n.DS_Store\n",
        }
    }

    fn entries_to_append(issue: &Issue) -> Vec<String> {
        match issue.id.as_str() {
            "CFG-003" | "SEC-003" => vec![".env".to_string()],
            "NJS-050" => vec![".env*.local".to_string()],
            "SYM-050" | "FLT-053" => {
                // Parse from title: ".gitignore missing: var/, vendor/"
                if let Some(suffix) = issue.title.strip_prefix(".gitignore missing: ") {
                    suffix.split(", ").map(|s| s.trim().to_string()).collect()
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }
}

impl Fixer for GitignoreFixer {
    fn handles(&self) -> &[&str] {
        &[
            "STR-003", "CFG-003", "SEC-003", "SYM-050", "FLT-053", "NJS-050",
        ]
    }

    fn describe(&self, issue: &Issue, project: &Project) -> String {
        match issue.id.as_str() {
            "STR-003" => {
                format!(
                    "Create .gitignore with {} template",
                    project.detected.framework
                )
            }
            _ => {
                let entries = Self::entries_to_append(issue);
                format!("Append to .gitignore: {}", entries.join(", "))
            }
        }
    }

    fn apply(&self, issue: &Issue, project: &Project) -> Result<FixResult> {
        let gitignore_path = project.path.join(".gitignore");

        match issue.id.as_str() {
            "STR-003" => {
                if gitignore_path.exists() {
                    return Ok(FixResult::Skipped {
                        reason: ".gitignore already exists".to_string(),
                    });
                }
                let template = Self::gitignore_template(&project.detected.framework);
                fs::write(&gitignore_path, template)?;
                Ok(FixResult::Applied {
                    description: format!(
                        "Created .gitignore with {} template",
                        project.detected.framework
                    ),
                })
            }
            _ => {
                let entries = Self::entries_to_append(issue);
                if entries.is_empty() {
                    return Ok(FixResult::Skipped {
                        reason: "No entries to append".to_string(),
                    });
                }

                let mut content = fs::read_to_string(&gitignore_path).unwrap_or_default();
                let mut added = Vec::new();

                for entry in &entries {
                    if !content.lines().any(|l| l.trim() == entry.as_str()) {
                        if !content.ends_with('\n') && !content.is_empty() {
                            content.push('\n');
                        }
                        content.push_str(entry);
                        content.push('\n');
                        added.push(entry.clone());
                    }
                }

                if added.is_empty() {
                    return Ok(FixResult::Skipped {
                        reason: "All entries already present in .gitignore".to_string(),
                    });
                }

                fs::write(&gitignore_path, content)?;
                Ok(FixResult::Applied {
                    description: format!("Added to .gitignore: {}", added.join(", ")),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::traits::{AnalyzerCategory, Severity};
    use crate::frameworks::detector::{DetectedProject, Language};
    use std::fs as stdfs;
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
    fn test_creates_new_gitignore() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::Symfony);
        let issue = make_issue("STR-003", "Missing .gitignore");

        let fixer = GitignoreFixer;
        let result = fixer.apply(&issue, &project).unwrap();

        assert!(matches!(result, FixResult::Applied { .. }));
        let content = stdfs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("vendor/"));
        assert!(content.contains("var/"));
        assert!(content.contains(".env"));
    }

    #[test]
    fn test_appends_to_existing_gitignore() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join(".gitignore"), "node_modules/\n").unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let issue = make_issue("CFG-003", ".env file found in project root");

        let fixer = GitignoreFixer;
        let result = fixer.apply(&issue, &project).unwrap();

        assert!(matches!(result, FixResult::Applied { .. }));
        let content = stdfs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules/"));
        assert!(content.contains(".env"));
    }

    #[test]
    fn test_skips_when_entry_already_present() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join(".gitignore"), ".env\n").unwrap();
        let project = make_project(&tmp, Framework::Unknown);
        let issue = make_issue("CFG-003", ".env file found in project root");

        let fixer = GitignoreFixer;
        let result = fixer.apply(&issue, &project).unwrap();

        assert!(matches!(result, FixResult::Skipped { .. }));
    }

    #[test]
    fn test_appends_symfony_gitignore_entries() {
        let tmp = TempDir::new().unwrap();
        stdfs::write(tmp.path().join(".gitignore"), ".env\n").unwrap();
        let project = make_project(&tmp, Framework::Symfony);
        let issue = make_issue("SYM-050", ".gitignore missing: var/, vendor/");

        let fixer = GitignoreFixer;
        let result = fixer.apply(&issue, &project).unwrap();

        assert!(matches!(result, FixResult::Applied { .. }));
        let content = stdfs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("var/"));
        assert!(content.contains("vendor/"));
    }

    #[test]
    fn test_creates_rust_gitignore_template() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp, Framework::RustCargo);
        let issue = make_issue("STR-003", "Missing .gitignore");

        let fixer = GitignoreFixer;
        let result = fixer.apply(&issue, &project).unwrap();

        assert!(matches!(result, FixResult::Applied { .. }));
        let content = stdfs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("target/"));
    }
}
