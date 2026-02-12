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

    /// Only fix issues matching these IDs (comma-separated, e.g. STR-001,STR-003)
    #[arg(long, value_delimiter = ',')]
    pub only: Option<Vec<String>>,
}

pub async fn execute(args: &FixArgs) -> Result<()> {
    let project = Project::new(&args.path)?;
    let scanner = default_scanner();

    let progress = crate::cli::progress::ScanProgress::new();
    let result = scanner
        .scan_with_progress(&project, |name| {
            progress.set_analyzer(name);
        })
        .await?;
    progress.finish();

    let mut fixable_issues: Vec<_> = result.issues.iter().filter(|i| i.auto_fixable).collect();

    if let Some(ref only) = args.only {
        fixable_issues.retain(|i| only.contains(&i.id));
    }

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

#[cfg(test)]
mod tests {
    use crate::analyzers::traits::{AnalyzerCategory, Issue, Severity};

    #[test]
    fn test_only_flag_filters_issues() {
        let issues = vec![
            Issue {
                id: "STR-001".to_string(),
                analyzer: "structure".to_string(),
                category: AnalyzerCategory::Structure,
                severity: Severity::High,
                title: "Missing src/".to_string(),
                description: "No src/ directory".to_string(),
                file: None,
                line: None,
                suggestion: None,
                auto_fixable: true,
                references: vec![],
            },
            Issue {
                id: "STR-003".to_string(),
                analyzer: "structure".to_string(),
                category: AnalyzerCategory::Structure,
                severity: Severity::Medium,
                title: "Missing .gitignore".to_string(),
                description: "No .gitignore".to_string(),
                file: None,
                line: None,
                suggestion: None,
                auto_fixable: true,
                references: vec![],
            },
            Issue {
                id: "CFG-002".to_string(),
                analyzer: "config_files".to_string(),
                category: AnalyzerCategory::Configuration,
                severity: Severity::Low,
                title: "Missing .editorconfig".to_string(),
                description: "No .editorconfig".to_string(),
                file: None,
                line: None,
                suggestion: None,
                auto_fixable: true,
                references: vec![],
            },
        ];

        let only = vec!["STR-001".to_string()];
        let mut fixable: Vec<_> = issues.iter().filter(|i| i.auto_fixable).collect();
        fixable.retain(|i| only.iter().any(|id| i.id == *id));

        assert_eq!(fixable.len(), 1);
        assert_eq!(fixable[0].id, "STR-001");
    }
}
