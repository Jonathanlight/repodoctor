use anyhow::Result;

use crate::analyzers::traits::Severity;
use crate::core::scanner::ScanResult;
use crate::core::score::Grade;

use super::traits::Reporter;

pub struct HtmlReporter;

impl Reporter for HtmlReporter {
    fn name(&self) -> &str {
        "html"
    }

    fn extension(&self) -> &str {
        "html"
    }

    fn generate(&self, result: &ScanResult) -> Result<String> {
        Ok(render_html(result))
    }
}

fn grade_color(grade: Grade) -> &'static str {
    match grade {
        Grade::A => "#4caf50",
        Grade::B => "#2196f3",
        Grade::C => "#ff9800",
        Grade::D => "#f44336",
        Grade::F => "#9e0000",
    }
}

fn severity_color(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "#d32f2f",
        Severity::High => "#f57c00",
        Severity::Medium => "#1976d2",
        Severity::Low => "#757575",
        Severity::Info => "#9e9e9e",
    }
}

fn score_bar_color(score: u8) -> &'static str {
    match score {
        80..=100 => "#4caf50",
        60..=79 => "#ff9800",
        _ => "#f44336",
    }
}

fn render_html(result: &ScanResult) -> String {
    let mut html = String::with_capacity(8192);

    // Header
    html.push_str(&format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>RepoDoctor Report - {}</title>
<style>
{}
</style>
</head>
<body>
<div class="container">
"#,
        escape_html(&result.project.path.to_string_lossy()),
        CSS
    ));

    // Title & project info
    html.push_str(&format!(
        r#"<h1>RepoDoctor Health Report</h1>
<div class="project-info">
  <p><strong>Project:</strong> {}</p>
  <p><strong>Framework:</strong> {}{}</p>
  <p><strong>Scan duration:</strong> {:.1}s</p>
</div>
"#,
        escape_html(&result.project.path.to_string_lossy()),
        result.project.detected.framework,
        result
            .project
            .detected
            .version
            .as_ref()
            .map(|v| format!(" {}", v))
            .unwrap_or_default(),
        result.duration.as_secs_f64(),
    ));

    // Health score
    let color = grade_color(result.score.grade);
    html.push_str(&format!(
        r#"<div class="score-section">
  <div class="score-circle" style="border-color: {}">
    <span class="score-value">{}</span>
    <span class="score-label">/ 100</span>
  </div>
  <div class="grade" style="color: {}">Grade {}</div>
</div>
"#,
        color, result.score.total, color, result.score.grade,
    ));

    // Category breakdown
    html.push_str(r#"<h2>Category Breakdown</h2>
<table class="breakdown">
<thead><tr><th>Category</th><th>Score</th><th>Issues</th><th>Status</th></tr></thead>
<tbody>
"#);

    for cat in &result.score.breakdown {
        let status = match cat.score {
            80..=100 => ("Good", "#4caf50"),
            60..=79 => ("Needs attention", "#ff9800"),
            _ => ("Poor", "#f44336"),
        };
        let bar_color = score_bar_color(cat.score);
        html.push_str(&format!(
            r#"<tr>
  <td>{}</td>
  <td><div class="bar-container"><div class="bar" style="width:{}%;background:{}"></div></div>{}/100</td>
  <td>{}</td>
  <td style="color:{}">{}</td>
</tr>
"#,
            cat.name,
            cat.score,
            bar_color,
            cat.score,
            cat.issues_count,
            status.1,
            status.0,
        ));
    }

    html.push_str("</tbody></table>\n");

    // Issues
    let severity_groups = [
        (Severity::Critical, "Critical"),
        (Severity::High, "High"),
        (Severity::Medium, "Medium"),
        (Severity::Low, "Low"),
        (Severity::Info, "Info"),
    ];

    html.push_str("<h2>Issues</h2>\n");

    let mut has_issues = false;
    for (severity, label) in &severity_groups {
        let group: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.severity == *severity)
            .collect();

        if group.is_empty() {
            continue;
        }
        has_issues = true;

        let color = severity_color(*severity);
        html.push_str(&format!(
            "<h3 style=\"color:{}\">{} ({})</h3>\n",
            color,
            label,
            group.len()
        ));

        for issue in &group {
            html.push_str(&format!(
                r#"<div class="issue">
  <div class="issue-header">
    <span class="issue-id" style="background:{}">{}</span>
    <span class="issue-title">{}</span>
    {}
  </div>
"#,
                color,
                issue.id,
                escape_html(&issue.title),
                if issue.auto_fixable {
                    "<span class=\"fixable\">Auto-fixable</span>"
                } else {
                    ""
                },
            ));

            if let Some(file) = &issue.file {
                html.push_str(&format!(
                    "  <p class=\"issue-file\">File: {}{}</p>\n",
                    escape_html(&file.to_string_lossy()),
                    issue
                        .line
                        .map(|l| format!(" (line {})", l))
                        .unwrap_or_default(),
                ));
            }

            if let Some(suggestion) = &issue.suggestion {
                html.push_str(&format!(
                    "  <p class=\"issue-suggestion\">Suggestion: {}</p>\n",
                    escape_html(suggestion),
                ));
            }

            html.push_str("</div>\n");
        }
    }

    if !has_issues {
        html.push_str("<p class=\"no-issues\">No issues found!</p>\n");
    }

    // Summary
    let total = result.issues.len();
    let critical = result
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .count();
    let high = result
        .issues
        .iter()
        .filter(|i| i.severity == Severity::High)
        .count();
    let fixable = result.issues.iter().filter(|i| i.auto_fixable).count();

    html.push_str(&format!(
        r#"<div class="summary">
  <h2>Summary</h2>
  <p>{} issues found ({} critical, {} high)</p>
  <p>{} auto-fixable issues</p>
</div>
"#,
        total, critical, high, fixable,
    ));

    // Footer
    html.push_str(
        r#"<footer>Generated by RepoDoctor v0.1.0</footer>
</div>
</body>
</html>
"#,
    );

    html
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const CSS: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
       line-height: 1.6; color: #333; background: #f5f5f5; }
.container { max-width: 900px; margin: 0 auto; padding: 2rem; background: #fff;
             min-height: 100vh; box-shadow: 0 0 20px rgba(0,0,0,0.05); }
h1 { margin-bottom: 1rem; color: #1a1a1a; }
h2 { margin: 2rem 0 1rem; color: #1a1a1a; border-bottom: 2px solid #eee; padding-bottom: 0.5rem; }
h3 { margin: 1.5rem 0 0.5rem; }
.project-info { background: #f8f9fa; padding: 1rem 1.5rem; border-radius: 8px; margin-bottom: 2rem; }
.project-info p { margin: 0.25rem 0; }
.score-section { text-align: center; margin: 2rem 0; }
.score-circle { display: inline-flex; flex-direction: column; align-items: center;
                justify-content: center; width: 120px; height: 120px; border-radius: 50%;
                border: 6px solid; }
.score-value { font-size: 2.5rem; font-weight: bold; line-height: 1; }
.score-label { font-size: 0.85rem; color: #666; }
.grade { font-size: 1.5rem; font-weight: bold; margin-top: 0.5rem; }
.breakdown { width: 100%; border-collapse: collapse; margin: 1rem 0; }
.breakdown th, .breakdown td { padding: 0.75rem 1rem; text-align: left; border-bottom: 1px solid #eee; }
.breakdown th { background: #f8f9fa; font-weight: 600; }
.bar-container { display: inline-block; width: 80px; height: 8px; background: #eee;
                 border-radius: 4px; margin-right: 0.5rem; vertical-align: middle; }
.bar { height: 100%; border-radius: 4px; }
.issue { background: #f8f9fa; padding: 1rem 1.5rem; border-radius: 8px; margin: 0.5rem 0;
         border-left: 4px solid #ddd; }
.issue-header { display: flex; align-items: center; gap: 0.75rem; flex-wrap: wrap; }
.issue-id { color: #fff; padding: 0.15rem 0.5rem; border-radius: 4px; font-size: 0.85rem;
            font-weight: 600; }
.issue-title { font-weight: 500; }
.fixable { background: #e8f5e9; color: #2e7d32; padding: 0.1rem 0.5rem; border-radius: 4px;
           font-size: 0.8rem; }
.issue-file { margin-top: 0.5rem; font-size: 0.9rem; color: #666; }
.issue-suggestion { margin-top: 0.25rem; font-size: 0.9rem; color: #555; font-style: italic; }
.no-issues { color: #4caf50; font-weight: 500; font-size: 1.1rem; }
.summary { background: #f8f9fa; padding: 1.5rem; border-radius: 8px; margin-top: 2rem; }
.summary p { margin: 0.25rem 0; }
footer { margin-top: 2rem; padding-top: 1rem; border-top: 1px solid #eee; color: #999;
         font-size: 0.85rem; text-align: center; }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::traits::{AnalyzerCategory, Issue};
    use crate::core::project::Project;
    use crate::core::score::HealthScore;
    use crate::frameworks::detector::{DetectedProject, Framework, Language};
    use std::time::Duration;

    fn make_result(issues: Vec<Issue>) -> ScanResult {
        let score = HealthScore::calculate(&issues);
        ScanResult {
            project: Project {
                path: "/tmp/test-project".into(),
                detected: DetectedProject {
                    framework: Framework::RustCargo,
                    language: Language::Rust,
                    version: Some("0.1.0".to_string()),
                    package_manager: None,
                    has_git: true,
                    has_ci: None,
                },
            },
            issues,
            score,
            duration: Duration::from_millis(1234),
        }
    }

    fn make_issue(id: &str, severity: Severity) -> Issue {
        Issue {
            id: id.to_string(),
            analyzer: "test".to_string(),
            category: AnalyzerCategory::Structure,
            severity,
            title: "Test issue".to_string(),
            description: "A test issue".to_string(),
            file: None,
            line: None,
            suggestion: Some("Fix it".to_string()),
            auto_fixable: true,
            references: vec![],
        }
    }

    #[test]
    fn test_html_report_contains_structure() {
        let result = make_result(vec![make_issue("TST-001", Severity::High)]);
        let reporter = HtmlReporter;
        let html = reporter.generate(&result).unwrap();

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("RepoDoctor Health Report"));
        assert!(html.contains("/tmp/test-project"));
        assert!(html.contains("Rust/Cargo"));
        assert!(html.contains("TST-001"));
        assert!(html.contains("Auto-fixable"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn test_html_report_no_issues() {
        let result = make_result(vec![]);
        let reporter = HtmlReporter;
        let html = reporter.generate(&result).unwrap();

        assert!(html.contains("100"));
        assert!(html.contains("Grade A"));
        assert!(html.contains("No issues found!"));
    }

    #[test]
    fn test_html_escapes_special_chars() {
        let html = escape_html("<script>alert('xss')</script>");
        assert!(!html.contains('<'));
        assert!(html.contains("&lt;"));
    }
}
