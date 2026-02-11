use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::analyzers::traits::{Issue, Severity};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub severity_threshold: Option<String>,
    pub ignore: Option<IgnoreConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IgnoreConfig {
    pub paths: Option<Vec<String>>,
    pub rules: Option<Vec<String>>,
}

impl Config {
    pub fn min_severity(&self) -> Severity {
        match self.severity_threshold.as_deref() {
            Some("critical") => Severity::Critical,
            Some("high") => Severity::High,
            Some("medium") => Severity::Medium,
            Some("low") => Severity::Low,
            _ => Severity::Info,
        }
    }

    pub fn is_rule_ignored(&self, rule_id: &str) -> bool {
        self.ignore
            .as_ref()
            .and_then(|ig| ig.rules.as_ref())
            .map(|rules| rules.iter().any(|r| r == rule_id))
            .unwrap_or(false)
    }

    pub fn is_path_ignored(&self, file_path: &str) -> bool {
        self.ignore
            .as_ref()
            .and_then(|ig| ig.paths.as_ref())
            .map(|paths| paths.iter().any(|p| file_path.starts_with(p.trim_end_matches('/'))))
            .unwrap_or(false)
    }

    pub fn filter_issues(&self, issues: Vec<Issue>) -> Vec<Issue> {
        let min_sev = self.min_severity();
        issues
            .into_iter()
            .filter(|issue| {
                if issue.severity < min_sev {
                    return false;
                }
                if self.is_rule_ignored(&issue.id) {
                    return false;
                }
                if let Some(file) = &issue.file {
                    if self.is_path_ignored(&file.to_string_lossy()) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
}

impl Config {
    pub fn load(project_path: &Path) -> Self {
        let config_path = project_path.join(".repodoctor.yml");
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = serde_yaml::from_str::<Config>(&content) {
                    return config;
                }
            }
        }
        Config::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::traits::AnalyzerCategory;
    use std::fs;
    use tempfile::TempDir;

    fn make_issue(id: &str, severity: Severity, file: Option<&str>) -> Issue {
        Issue {
            id: id.to_string(),
            analyzer: "test".to_string(),
            category: AnalyzerCategory::Structure,
            severity,
            title: "Test".to_string(),
            description: "Test".to_string(),
            file: file.map(|f| f.into()),
            line: None,
            suggestion: None,
            auto_fixable: false,
            references: vec![],
        }
    }

    #[test]
    fn test_default_config_when_no_file() {
        let tmp = TempDir::new().unwrap();
        let config = Config::load(tmp.path());
        assert!(config.severity_threshold.is_none());
        assert!(config.ignore.is_none());
    }

    #[test]
    fn test_load_config_from_file() {
        let tmp = TempDir::new().unwrap();
        let yaml = "severity_threshold: high\nignore:\n  rules:\n    - DOC-003\n";
        fs::write(tmp.path().join(".repodoctor.yml"), yaml).unwrap();
        let config = Config::load(tmp.path());
        assert_eq!(config.severity_threshold, Some("high".to_string()));
        let ignore = config.ignore.unwrap();
        assert_eq!(ignore.rules.unwrap(), vec!["DOC-003".to_string()]);
    }

    #[test]
    fn test_min_severity_default() {
        let config = Config::default();
        assert_eq!(config.min_severity(), Severity::Info);
    }

    #[test]
    fn test_min_severity_high() {
        let config = Config {
            severity_threshold: Some("high".to_string()),
            ignore: None,
        };
        assert_eq!(config.min_severity(), Severity::High);
    }

    #[test]
    fn test_is_rule_ignored() {
        let config = Config {
            severity_threshold: None,
            ignore: Some(IgnoreConfig {
                paths: None,
                rules: Some(vec!["DOC-003".to_string(), "STR-005".to_string()]),
            }),
        };
        assert!(config.is_rule_ignored("DOC-003"));
        assert!(config.is_rule_ignored("STR-005"));
        assert!(!config.is_rule_ignored("SEC-001"));
    }

    #[test]
    fn test_is_path_ignored() {
        let config = Config {
            severity_threshold: None,
            ignore: Some(IgnoreConfig {
                paths: Some(vec!["vendor/".to_string(), "node_modules/".to_string()]),
                rules: None,
            }),
        };
        assert!(config.is_path_ignored("vendor/autoload.php"));
        assert!(config.is_path_ignored("node_modules/package/index.js"));
        assert!(!config.is_path_ignored("src/main.rs"));
    }

    #[test]
    fn test_filter_issues_by_severity() {
        let config = Config {
            severity_threshold: Some("medium".to_string()),
            ignore: None,
        };
        let issues = vec![
            make_issue("A", Severity::Critical, None),
            make_issue("B", Severity::High, None),
            make_issue("C", Severity::Medium, None),
            make_issue("D", Severity::Low, None),
            make_issue("E", Severity::Info, None),
        ];
        let filtered = config.filter_issues(issues);
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].id, "A");
        assert_eq!(filtered[1].id, "B");
        assert_eq!(filtered[2].id, "C");
    }

    #[test]
    fn test_filter_issues_by_rule() {
        let config = Config {
            severity_threshold: None,
            ignore: Some(IgnoreConfig {
                paths: None,
                rules: Some(vec!["STR-005".to_string()]),
            }),
        };
        let issues = vec![
            make_issue("STR-001", Severity::High, None),
            make_issue("STR-005", Severity::Info, None),
        ];
        let filtered = config.filter_issues(issues);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "STR-001");
    }

    #[test]
    fn test_filter_issues_by_path() {
        let config = Config {
            severity_threshold: None,
            ignore: Some(IgnoreConfig {
                paths: Some(vec!["vendor/".to_string()]),
                rules: None,
            }),
        };
        let issues = vec![
            make_issue("A", Severity::High, Some("vendor/autoload.php")),
            make_issue("B", Severity::High, Some("src/main.rs")),
            make_issue("C", Severity::High, None),
        ];
        let filtered = config.filter_issues(issues);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, "B");
        assert_eq!(filtered[1].id, "C");
    }
}
