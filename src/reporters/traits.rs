use anyhow::Result;

use crate::core::scanner::ScanResult;

pub trait Reporter: Send + Sync {
    /// Reporter name for display
    fn name(&self) -> &str;

    /// File extension for the output file
    fn extension(&self) -> &str;

    /// Generate the report content as a string
    fn generate(&self, result: &ScanResult) -> Result<String>;
}
