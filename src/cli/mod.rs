pub mod commands;
pub mod output;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "repodoctor", version, about = "Diagnose the health of your repository")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Scan a project for health issues
    Scan(commands::scan::ScanArgs),
    /// Auto-fix detected issues
    Fix(commands::fix::FixArgs),
    /// Generate a health report
    Report(commands::report::ReportArgs),
    /// Initialize a .repodoctor.yml configuration file
    Init(commands::init::InitArgs),
}
