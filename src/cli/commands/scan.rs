use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::analyzers::traits::Severity;
use crate::cli::output::OutputFormatter;
use crate::core::project::Project;
use crate::core::scanner::default_scanner;

#[derive(Args, Debug)]
pub struct ScanArgs {
    /// Path to the project to scan (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output format
    #[arg(long, default_value = "table", value_parser = ["table", "json"])]
    pub format: String,

    /// Minimum severity to display
    #[arg(long, value_parser = ["info", "low", "medium", "high", "critical"])]
    pub severity: Option<String>,

    /// CI mode: exit with code 1 if issues at or above threshold are found
    #[arg(long)]
    pub ci: bool,

    /// Severity threshold for CI failure (default: high)
    #[arg(long, default_value = "high", value_parser = ["low", "medium", "high", "critical"])]
    pub fail_on: String,

    /// Only run specific analyzers (comma-separated: structure,deps,config,security,testing,docs)
    #[arg(long, value_delimiter = ',')]
    pub only: Option<Vec<String>>,
}

impl ScanArgs {
    fn min_severity(&self) -> Severity {
        match self.severity.as_deref() {
            Some("critical") => Severity::Critical,
            Some("high") => Severity::High,
            Some("medium") => Severity::Medium,
            Some("low") => Severity::Low,
            _ => Severity::Info,
        }
    }

    fn fail_severity(&self) -> Severity {
        match self.fail_on.as_str() {
            "critical" => Severity::Critical,
            "medium" => Severity::Medium,
            "low" => Severity::Low,
            _ => Severity::High,
        }
    }
}

fn expand_analyzer_name(name: &str) -> &str {
    match name.trim() {
        "deps" | "dependencies" => "dependencies",
        "config" | "configuration" => "config_files",
        "docs" | "documentation" => "documentation",
        "struct" | "structure" => "structure",
        "sec" | "security" => "security",
        "test" | "testing" => "testing",
        "symfony" => "symfony",
        "flutter" => "flutter",
        "nextjs" | "next" => "nextjs",
        "laravel" => "laravel",
        "rust" | "cargo" | "rust_cargo" => "rust_cargo",
        other => other,
    }
}

pub async fn execute(args: &ScanArgs) -> Result<()> {
    let project = Project::new(&args.path)?;
    let scanner = default_scanner();
    let mut result = if args.format == "table" {
        let progress = crate::cli::progress::ScanProgress::new();
        let res = scanner
            .scan_with_progress(&project, |name| {
                progress.set_analyzer(name);
            })
            .await?;
        progress.finish();
        res
    } else {
        scanner.scan(&project).await?
    };

    let min_severity = args.min_severity();
    result.issues.retain(|i| i.severity >= min_severity);

    if let Some(only) = &args.only {
        let allowed: Vec<&str> = only.iter().map(|n| expand_analyzer_name(n)).collect();
        result.issues.retain(|i| allowed.contains(&i.analyzer.as_str()));
        // Recalculate score with filtered issues
        result.score = crate::core::score::HealthScore::calculate(&result.issues);
    }

    let formatter = OutputFormatter::new(&args.format);
    formatter.display(&result);

    if args.ci {
        let threshold = args.fail_severity();
        let failing_count = result.issues.iter().filter(|i| i.severity >= threshold).count();
        if failing_count > 0 {
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_severity_default() {
        let args = ScanArgs {
            path: PathBuf::from("."),
            format: "table".to_string(),
            severity: None,
            ci: false,
            fail_on: "high".to_string(),
            only: None,
        };
        assert_eq!(args.min_severity(), Severity::Info);
    }

    #[test]
    fn test_min_severity_critical() {
        let args = ScanArgs {
            path: PathBuf::from("."),
            format: "table".to_string(),
            severity: Some("critical".to_string()),
            ci: false,
            fail_on: "high".to_string(),
            only: None,
        };
        assert_eq!(args.min_severity(), Severity::Critical);
    }

    #[test]
    fn test_fail_severity_default() {
        let args = ScanArgs {
            path: PathBuf::from("."),
            format: "table".to_string(),
            severity: None,
            ci: true,
            fail_on: "high".to_string(),
            only: None,
        };
        assert_eq!(args.fail_severity(), Severity::High);
    }

    #[test]
    fn test_fail_severity_critical() {
        let args = ScanArgs {
            path: PathBuf::from("."),
            format: "table".to_string(),
            severity: None,
            ci: true,
            fail_on: "critical".to_string(),
            only: None,
        };
        assert_eq!(args.fail_severity(), Severity::Critical);
    }

    #[test]
    fn test_expand_analyzer_name_aliases() {
        assert_eq!(expand_analyzer_name("deps"), "dependencies");
        assert_eq!(expand_analyzer_name("config"), "config_files");
        assert_eq!(expand_analyzer_name("docs"), "documentation");
        assert_eq!(expand_analyzer_name("sec"), "security");
        assert_eq!(expand_analyzer_name("test"), "testing");
        assert_eq!(expand_analyzer_name("next"), "nextjs");
        assert_eq!(expand_analyzer_name("structure"), "structure");
        assert_eq!(expand_analyzer_name("laravel"), "laravel");
        assert_eq!(expand_analyzer_name("rust"), "rust_cargo");
        assert_eq!(expand_analyzer_name("cargo"), "rust_cargo");
        assert_eq!(expand_analyzer_name("rust_cargo"), "rust_cargo");
    }
}
