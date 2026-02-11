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
}

pub async fn execute(args: &ScanArgs) -> Result<()> {
    let project = Project::new(&args.path)?;
    let scanner = default_scanner();
    let mut result = scanner.scan(&project).await?;

    let min_severity = args.min_severity();
    result.issues.retain(|i| i.severity >= min_severity);

    let formatter = OutputFormatter::new(&args.format);
    formatter.display(&result);

    Ok(())
}
