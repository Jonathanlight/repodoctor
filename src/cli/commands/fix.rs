use anyhow::Result;
use clap::Args;
use colored::Colorize;
use std::path::PathBuf;

use crate::core::project::Project;
use crate::core::scanner::default_scanner;
use crate::fixers::default_registry;
use crate::fixers::registry::FixOutcome;

#[derive(Args, Debug)]
pub struct FixArgs {
    /// Path to the project to fix (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Print what would be fixed without modifying files
    #[arg(long)]
    pub dry_run: bool,

    /// Apply all fixes without prompting
    #[arg(long)]
    pub auto: bool,
}

pub async fn execute(args: &FixArgs) -> Result<()> {
    let project = Project::new(&args.path)?;
    let scanner = default_scanner();
    let result = scanner.scan(&project).await?;

    let fixable_issues: Vec<_> = result.issues.iter().filter(|i| i.auto_fixable).collect();

    if fixable_issues.is_empty() {
        println!("{}", "No auto-fixable issues found.".green());
        return Ok(());
    }

    println!(
        "{} auto-fixable issue(s) found.\n",
        fixable_issues.len().to_string().bold()
    );

    let registry = default_registry();
    let results = registry.apply_fixes(&fixable_issues, &project, args.dry_run);

    let mut applied = 0;
    let mut skipped = 0;

    for (id, outcome) in &results {
        match outcome {
            FixOutcome::Applied(desc) => {
                println!("  {} [{}] {}", "FIXED".green(), id, desc);
                applied += 1;
            }
            FixOutcome::Skipped(reason) => {
                println!("  {} [{}] {}", "SKIP".yellow(), id, reason);
                skipped += 1;
            }
            FixOutcome::DryRun(desc) => {
                println!("  {} [{}] {}", "DRY-RUN".cyan(), id, desc);
            }
            FixOutcome::Error(err) => {
                println!("  {} [{}] {}", "ERROR".red(), id, err);
                skipped += 1;
            }
        }
    }

    if !args.dry_run {
        println!("\n{} fixed, {} skipped.", applied, skipped);
    }

    Ok(())
}
