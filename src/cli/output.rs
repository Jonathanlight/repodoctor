use colored::*;

use crate::analyzers::traits::Severity;
use crate::core::scanner::ScanResult;
use crate::core::score::Grade;

pub struct OutputFormatter {
    format: String,
}

impl OutputFormatter {
    pub fn new(format: &str) -> Self {
        Self {
            format: format.to_string(),
        }
    }

    pub fn display(&self, result: &ScanResult) {
        match self.format.as_str() {
            "json" => self.display_json(result),
            _ => self.display_table(result),
        }
    }

    fn display_json(&self, result: &ScanResult) {
        let output = serde_json::json!({
            "project": {
                "path": result.project.path.to_string_lossy(),
                "framework": result.project.detected.framework,
                "language": result.project.detected.language,
                "version": result.project.detected.version,
            },
            "score": {
                "total": result.score.total,
                "grade": format!("{}", result.score.grade),
                "breakdown": result.score.breakdown,
            },
            "issues": result.issues,
            "duration_ms": result.duration.as_millis(),
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    }

    fn display_table(&self, result: &ScanResult) {
        // Header
        println!();
        println!("{}", "RepoDoctor v0.1.0".bold());
        println!("{}", "─".repeat(64));
        println!();

        // Project info
        println!(
            "  Project:  {}",
            result.project.path.to_string_lossy().cyan()
        );
        println!(
            "  Detected: {}{}",
            result.project.detected.framework.to_string().green(),
            result
                .project
                .detected
                .version
                .as_ref()
                .map(|v| format!(" {}", v))
                .unwrap_or_default()
        );
        println!(
            "  Scan completed in {:.1}s",
            result.duration.as_secs_f64()
        );
        println!();
        println!("{}", "─".repeat(64));

        // Health score
        let grade_color = match result.score.grade {
            Grade::A => "green",
            Grade::B => "blue",
            Grade::C => "yellow",
            Grade::D => "red",
            Grade::F => "red",
        };
        let score_str = format!(
            "HEALTH SCORE: {}/100 (Grade {})",
            result.score.total, result.score.grade
        );
        println!();
        match grade_color {
            "green" => println!("  {}", score_str.green().bold()),
            "blue" => println!("  {}", score_str.blue().bold()),
            "yellow" => println!("  {}", score_str.yellow().bold()),
            _ => println!("  {}", score_str.red().bold()),
        }
        println!();

        // Category breakdown table
        println!(
            "  {:<18} {:<8} {:<8} {}",
            "Category".bold(),
            "Score".bold(),
            "Issues".bold(),
            "Status".bold()
        );
        println!("  {}", "─".repeat(58));

        for cat in &result.score.breakdown {
            let status = match cat.score {
                80..=100 => "Good".green(),
                60..=79 => "Needs attention".yellow(),
                _ => "Poor".red(),
            };
            println!(
                "  {:<18} {:<8} {:<8} {}",
                cat.name,
                format!("{}/100", cat.score),
                cat.issues_count,
                status,
            );
        }

        println!();
        println!("{}", "─".repeat(64));

        // Issues grouped by severity
        let severity_groups = [
            (Severity::Critical, "CRITICAL", Color::Red),
            (Severity::High, "HIGH", Color::Yellow),
            (Severity::Medium, "MEDIUM", Color::Blue),
            (Severity::Low, "LOW", Color::White),
            (Severity::Info, "INFO", Color::BrightBlack),
        ];

        for (severity, label, color) in &severity_groups {
            let group: Vec<_> = result
                .issues
                .iter()
                .filter(|i| i.severity == *severity)
                .collect();

            if group.is_empty() {
                continue;
            }

            println!();
            println!(
                "  {} ({})",
                label.color(*color).bold(),
                group.len()
            );
            println!();

            for issue in &group {
                println!(
                    "    {}  {}",
                    issue.id.color(*color).bold(),
                    issue.title
                );
                if let Some(file) = &issue.file {
                    println!(
                        "           File: {}{}",
                        file.to_string_lossy(),
                        issue
                            .line
                            .map(|l| format!(" (line {})", l))
                            .unwrap_or_default()
                    );
                }
                if let Some(suggestion) = &issue.suggestion {
                    println!("           Suggestion: {}", suggestion.dimmed());
                }
                if issue.auto_fixable {
                    println!("           {}", "Auto-fixable: Yes".green());
                }
                println!();
            }
        }

        // Summary
        println!("{}", "─".repeat(64));
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
        let medium = result
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Medium)
            .count();
        let low = result
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Low)
            .count();
        let fixable = result.issues.iter().filter(|i| i.auto_fixable).count();

        println!();
        println!("  SUMMARY");
        println!(
            "    {} issues found ({} critical, {} high, {} medium, {} low)",
            total, critical, high, medium, low
        );
        if fixable > 0 {
            println!("    {} auto-fixable issues", fixable);
        }
        println!();
    }
}
