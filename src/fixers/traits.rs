use anyhow::Result;

use crate::analyzers::traits::Issue;
use crate::core::project::Project;

pub enum FixResult {
    Applied { description: String },
    Skipped { reason: String },
}

pub trait Fixer: Send + Sync {
    /// Issue IDs this fixer handles
    fn handles(&self) -> &[&str];

    /// Describe what would be done (for dry-run)
    fn describe(&self, issue: &Issue, project: &Project) -> String;

    /// Apply the fix
    fn apply(&self, issue: &Issue, project: &Project) -> Result<FixResult>;
}
