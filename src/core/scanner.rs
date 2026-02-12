use anyhow::Result;
use std::time::{Duration, Instant};

use crate::analyzers::traits::{Analyzer, Issue};
use crate::core::config::Config;
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
        self.scan_with_progress(project, |_| {}).await
    }

    pub async fn scan_with_progress<F: Fn(&str)>(
        &self,
        project: &Project,
        on_analyzer: F,
    ) -> Result<ScanResult> {
        let start = Instant::now();
        let config = Config::load(&project.path);
        let mut all_issues: Vec<Issue> = Vec::new();

        for analyzer in &self.analyzers {
            if analyzer.applies_to(project) {
                on_analyzer(analyzer.name());
                let issues = analyzer.analyze(project).await?;
                all_issues.extend(issues);
            }
        }

        // Apply config filters (severity threshold, ignored rules/paths)
        all_issues = config.filter_issues(all_issues);

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
        Box::new(crate::analyzers::DependenciesAnalyzer),
        Box::new(crate::analyzers::ConfigAnalyzer),
        Box::new(crate::analyzers::SecurityAnalyzer),
        Box::new(crate::analyzers::TestingAnalyzer),
        Box::new(crate::analyzers::DocumentationAnalyzer),
        Box::new(crate::analyzers::SymfonyAnalyzer),
        Box::new(crate::analyzers::FlutterAnalyzer),
        Box::new(crate::analyzers::NextJsAnalyzer),
        Box::new(crate::analyzers::LaravelAnalyzer),
        Box::new(crate::analyzers::RustCargoAnalyzer),
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

    #[tokio::test]
    async fn test_scanner_respects_config_severity_threshold() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);

        // First scan without config to get baseline
        let scanner = default_scanner();
        let baseline = scanner.scan(&project).await.unwrap();
        let has_low = baseline.issues.iter().any(|i| {
            i.severity == crate::analyzers::traits::Severity::Low
                || i.severity == crate::analyzers::traits::Severity::Info
        });

        if has_low {
            // Now scan with config that filters low/info
            fs::write(
                tmp.path().join(".repodoctor.yml"),
                "severity_threshold: medium\n",
            )
            .unwrap();
            let result = scanner.scan(&project).await.unwrap();
            assert!(result.issues.iter().all(|i| {
                i.severity >= crate::analyzers::traits::Severity::Medium
            }));
            assert!(result.issues.len() < baseline.issues.len());
        }
    }

    #[tokio::test]
    async fn test_scan_with_progress() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("src")).unwrap();
        let project = make_project(&tmp);
        let scanner = default_scanner();
        let names = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let names_clone = names.clone();
        let result = scanner
            .scan_with_progress(&project, move |name| {
                names_clone.lock().unwrap().push(name.to_string());
            })
            .await
            .unwrap();
        assert!(result.duration.as_secs() < 10);
        let collected = names.lock().unwrap();
        assert!(!collected.is_empty(), "Progress callback should have been called");
    }

    #[tokio::test]
    async fn test_scanner_respects_config_ignored_rules() {
        let tmp = TempDir::new().unwrap();
        let project = make_project(&tmp);

        let scanner = default_scanner();
        let baseline = scanner.scan(&project).await.unwrap();

        if let Some(first_issue) = baseline.issues.first() {
            let rule_to_ignore = first_issue.id.clone();
            fs::write(
                tmp.path().join(".repodoctor.yml"),
                format!("ignore:\n  rules:\n    - {}\n", rule_to_ignore),
            )
            .unwrap();
            let result = scanner.scan(&project).await.unwrap();
            assert!(result.issues.iter().all(|i| i.id != rule_to_ignore));
        }
    }
}
