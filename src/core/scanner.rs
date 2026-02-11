use anyhow::Result;
use std::time::{Duration, Instant};

use crate::analyzers::traits::{Analyzer, Issue};
use crate::core::project::Project;
use crate::core::score::HealthScore;

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub project: Project,
    pub issues: Vec<Issue>,
    pub score: HealthScore,
    pub duration: Duration,
}

pub struct Scanner {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl Scanner {
    pub fn new(analyzers: Vec<Box<dyn Analyzer>>) -> Self {
        Self { analyzers }
    }

    pub async fn scan(&self, project: &Project) -> Result<ScanResult> {
        let start = Instant::now();
        let mut all_issues: Vec<Issue> = Vec::new();

        for analyzer in &self.analyzers {
            if analyzer.applies_to(project) {
                let issues = analyzer.analyze(project).await?;
                all_issues.extend(issues);
            }
        }

        // Sort issues by severity (Critical first)
        all_issues.sort_by(|a, b| b.severity.cmp(&a.severity));

        let score = HealthScore::calculate(&all_issues);
        let duration = start.elapsed();

        Ok(ScanResult {
            project: project.clone(),
            issues: all_issues,
            score,
            duration,
        })
    }
}

pub fn default_scanner() -> Scanner {
    let analyzers: Vec<Box<dyn Analyzer>> = vec![
        Box::new(crate::analyzers::StructureAnalyzer),
    ];
    Scanner::new(analyzers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::detector::{DetectedProject, Framework, Language};
    use std::fs;
    use tempfile::TempDir;

    fn make_project(tmp: &TempDir) -> Project {
        Project {
            path: tmp.path().to_path_buf(),
            detected: DetectedProject {
                framework: Framework::RustCargo,
                language: Language::Rust,
                version: None,
                package_manager: None,
                has_git: false,
                has_ci: None,
            },
        }
    }

    #[tokio::test]
    async fn test_scanner_produces_results() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("src")).unwrap();
        let project = make_project(&tmp);
        let scanner = default_scanner();
        let result = scanner.scan(&project).await.unwrap();
        assert!(result.duration.as_secs() < 10);
        assert!(result.score.total <= 100);
    }

    #[tokio::test]
    async fn test_scanner_finds_issues() {
        let tmp = TempDir::new().unwrap();
        // No src, no README, no .gitignore, no LICENSE -> many issues
        let project = make_project(&tmp);
        let scanner = default_scanner();
        let result = scanner.scan(&project).await.unwrap();
        assert!(!result.issues.is_empty());
    }

    #[tokio::test]
    async fn test_scanner_issues_sorted_by_severity() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);
        let scanner = default_scanner();
        let result = scanner.scan(&project).await.unwrap();
        for window in result.issues.windows(2) {
            assert!(window[0].severity >= window[1].severity);
        }
    }
}
