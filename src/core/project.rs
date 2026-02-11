use std::path::{Path, PathBuf};

use crate::frameworks::detector::{DetectedProject, FrameworkDetector};

#[derive(Debug, Clone)]
pub struct Project {
    pub path: PathBuf,
    pub detected: DetectedProject,
}

impl Project {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let canonical = path.canonicalize()?;
        let detected = FrameworkDetector::detect(&canonical);
        Ok(Self {
            path: canonical,
            detected,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::detector::Framework;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_project_new_detects_framework() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nversion = \"0.1.0\"\n").unwrap();
        let project = Project::new(tmp.path()).unwrap();
        assert_eq!(project.detected.framework, Framework::RustCargo);
    }

    #[test]
    fn test_project_new_unknown_framework() {
        let tmp = TempDir::new().unwrap();
        let project = Project::new(tmp.path()).unwrap();
        assert_eq!(project.detected.framework, Framework::Unknown);
    }
}
