use anyhow::Result;

use crate::core::scanner::ScanResult;
use crate::reporters::traits::Reporter;

pub struct JsonReporter;

impl Reporter for JsonReporter {
    fn name(&self) -> &str {
        "JSON"
    }

    fn extension(&self) -> &str {
        "json"
    }

    fn generate(&self, result: &ScanResult) -> Result<String> {
        let output = serde_json::json!({
            "project": {
                "path": result.project.path.to_string_lossy(),
                "framework": result.project.detected.framework,
                "language": result.project.detected.language,
                "version": result.project.detected.version,
                "package_manager": result.project.detected.package_manager,
                "has_git": result.project.detected.has_git,
                "has_ci": result.project.detected.has_ci,
            },
            "score": {
                "total": result.score.total,
                "grade": format!("{}", result.score.grade),
                "breakdown": result.score.breakdown,
            },
            "issues": result.issues,
            "summary": {
                "total_issues": result.issues.len(),
                "critical": result.issues.iter().filter(|i| i.severity == crate::analyzers::traits::Severity::Critical).count(),
                "high": result.issues.iter().filter(|i| i.severity == crate::analyzers::traits::Severity::High).count(),
                "medium": result.issues.iter().filter(|i| i.severity == crate::analyzers::traits::Severity::Medium).count(),
                "low": result.issues.iter().filter(|i| i.severity == crate::analyzers::traits::Severity::Low).count(),
                "info": result.issues.iter().filter(|i| i.severity == crate::analyzers::traits::Severity::Info).count(),
                "auto_fixable": result.issues.iter().filter(|i| i.auto_fixable).count(),
            },
            "duration_ms": result.duration.as_millis(),
        });
        Ok(serde_json::to_string_pretty(&output)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::traits::{AnalyzerCategory, Issue, Severity};
    use crate::core::project::Project;
    use crate::core::score::HealthScore;
    use crate::frameworks::detector::{DetectedProject, Framework, Language};
    use std::path::PathBuf;
    use std::time::Duration;

    fn make_result(issues: Vec<Issue>) -> ScanResult {
        ScanResult {
            project: Project {
                path: PathBuf::from("/tmp/test"),
                detected: DetectedProject {
                    framework: Framework::RustCargo,
                    language: Language::Rust,
                    version: Some("0.1.0".to_string()),
                    package_manager: None,
                    has_git: true,
                    has_ci: None,
                },
            },
            score: HealthScore::calculate(&issues),
            issues,
            duration: Duration::from_millis(42),
        }
    }

    #[test]
    fn test_json_report_valid_json() {
        let result = make_result(vec![]);
        let reporter = JsonReporter;
        let output = reporter.generate(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["score"]["total"], 100);
        assert_eq!(parsed["score"]["grade"], "A");
        assert_eq!(parsed["summary"]["total_issues"], 0);
    }

    #[test]
    fn test_json_report_with_issues() {
        let issues = vec![Issue {
            id: "STR-001".to_string(),
            analyzer: "structure".to_string(),
            category: AnalyzerCategory::Structure,
            severity: Severity::High,
            title: "Missing src/".to_string(),
            description: "No src directory".to_string(),
            file: None,
            line: None,
            suggestion: Some("Create src/".to_string()),
            auto_fixable: true,
            references: vec![],
        }];
        let result = make_result(issues);
        let reporter = JsonReporter;
        let output = reporter.generate(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["summary"]["total_issues"], 1);
        assert_eq!(parsed["summary"]["high"], 1);
        assert_eq!(parsed["summary"]["auto_fixable"], 1);
        assert_eq!(parsed["issues"][0]["id"], "STR-001");
    }

    #[test]
    fn test_json_report_project_info() {
        let result = make_result(vec![]);
        let reporter = JsonReporter;
        let output = reporter.generate(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["project"]["framework"], "RustCargo");
        assert_eq!(parsed["project"]["language"], "Rust");
        assert_eq!(parsed["project"]["version"], "0.1.0");
        assert_eq!(parsed["duration_ms"], 42);
    }

    #[test]
    fn test_json_reporter_metadata() {
        let reporter = JsonReporter;
        assert_eq!(reporter.name(), "JSON");
        assert_eq!(reporter.extension(), "json");
    }
}
