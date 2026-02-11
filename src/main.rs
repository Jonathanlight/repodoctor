mod analyzers;
mod cli;
mod core;
mod frameworks;
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
    }

    Ok(())
}
