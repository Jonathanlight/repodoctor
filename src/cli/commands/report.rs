use anyhow::Result;
use clap::Args;
use colored::Colorize;
use std::path::PathBuf;

use crate::core::project::Project;
use crate::core::scanner::default_scanner;
use crate::reporters::badge::BadgeGenerator;
use crate::reporters::html::HtmlReporter;
use crate::reporters::markdown::MarkdownReporter;
use crate::reporters::traits::Reporter;

#[derive(Args, Debug)]
pub struct ReportArgs {
    /// Path to the project to report on (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Report format
    #[arg(long, default_value = "html", value_parser = ["html", "markdown", "json"])]
    pub format: String,

    /// Output file path (auto-generated if not specified)
    #[arg(long, short)]
    pub output: Option<PathBuf>,

    /// Also generate a health badge SVG
    #[arg(long)]
    pub badge: bool,
}

pub async fn execute(args: &ReportArgs) -> Result<()> {
    let project = Project::new(&args.path)?;
    let scanner = default_scanner();
    let result = scanner.scan(&project).await?;

    let reporter: Box<dyn Reporter> = match args.format.as_str() {
        "markdown" => Box::new(MarkdownReporter),
        _ => Box::new(HtmlReporter),
    };

    let content = reporter.generate(&result)?;

    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("repodoctor-report.{}", reporter.extension())));

    std::fs::write(&output_path, &content)?;
    println!(
        "  {} {} report written to {}",
        "DONE".green(),
        reporter.name(),
        output_path.display()
    );

    if args.badge {
        let badge_svg = BadgeGenerator::generate(&result.score)?;
        let badge_path = PathBuf::from("repodoctor-badge.svg");
        std::fs::write(&badge_path, &badge_svg)?;
        println!(
            "  {} Badge SVG written to {}",
            "DONE".green(),
            badge_path.display()
        );
    }

    Ok(())
}
