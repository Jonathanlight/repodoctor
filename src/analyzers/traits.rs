use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::core::project::Project;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnalyzerCategory {
    Structure,
    Dependencies,
    Configuration,
    Testing,
    Security,
    Documentation,
}

impl std::fmt::Display for AnalyzerCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalyzerCategory::Structure => write!(f, "Structure"),
            AnalyzerCategory::Dependencies => write!(f, "Dependencies"),
            AnalyzerCategory::Configuration => write!(f, "Configuration"),
            AnalyzerCategory::Testing => write!(f, "Testing"),
            AnalyzerCategory::Security => write!(f, "Security"),
            AnalyzerCategory::Documentation => write!(f, "Documentation"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Info = 0,
    Low = 25,
    Medium = 50,
    High = 75,
    Critical = 100,
}

impl Severity {
    pub fn penalty(&self) -> u8 {
        match self {
            Severity::Critical => 25,
            Severity::High => 15,
            Severity::Medium => 8,
            Severity::Low => 3,
            Severity::Info => 0,
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "CRITICAL"),
            Severity::High => write!(f, "HIGH"),
            Severity::Medium => write!(f, "MEDIUM"),
            Severity::Low => write!(f, "LOW"),
            Severity::Info => write!(f, "INFO"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub analyzer: String,
    pub category: AnalyzerCategory,
    pub severity: Severity,
    pub title: String,
    pub description: String,
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
    pub suggestion: Option<String>,
    pub auto_fixable: bool,
    pub references: Vec<String>,
}

#[async_trait]
pub trait Analyzer: Send + Sync {
    fn name(&self) -> &'static str;
    #[allow(dead_code)]
    fn description(&self) -> &'static str;
    #[allow(dead_code)]
    fn category(&self) -> AnalyzerCategory;
    fn applies_to(&self, project: &Project) -> bool;
    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
        assert!(Severity::Low > Severity::Info);
    }

    #[test]
    fn test_severity_penalty() {
        assert_eq!(Severity::Critical.penalty(), 25);
        assert_eq!(Severity::High.penalty(), 15);
        assert_eq!(Severity::Medium.penalty(), 8);
        assert_eq!(Severity::Low.penalty(), 3);
        assert_eq!(Severity::Info.penalty(), 0);
    }

    #[test]
    fn test_issue_creation() {
        let issue = Issue {
            id: "TST-001".to_string(),
            analyzer: "test".to_string(),
            category: AnalyzerCategory::Testing,
            severity: Severity::High,
            title: "Test issue".to_string(),
            description: "A test issue".to_string(),
            file: Some(PathBuf::from("test.rs")),
            line: Some(42),
            suggestion: Some("Fix it".to_string()),
            auto_fixable: false,
            references: vec!["https://example.com".to_string()],
        };
        assert_eq!(issue.id, "TST-001");
        assert_eq!(issue.severity, Severity::High);
        assert!(issue.file.is_some());
    }
}
