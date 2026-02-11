mod analyzers;
mod cli;
mod core;
mod fixers;
mod frameworks;
mod reporters;
mod utils;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Scan(args) => {
            cli::commands::scan::execute(args).await?;
        }
        Commands::Fix(args) => {
            cli::commands::fix::execute(args).await?;
        }
        Commands::Report(args) => {
            cli::commands::report::execute(args).await?;
        }
        Commands::Init(args) => {
            cli::commands::init::execute(args).await?;
        }
    }

    Ok(())
}
