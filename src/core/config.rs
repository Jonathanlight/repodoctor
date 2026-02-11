#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub severity_threshold: Option<String>,
    pub ignore: Option<IgnoreConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnoreConfig {
    pub paths: Option<Vec<String>>,
    pub rules: Option<Vec<String>>,
}

impl Config {
    pub fn load(project_path: &Path) -> Self {
        let config_path = project_path.join(".repodoctor.yml");
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = serde_yaml::from_str::<Config>(&content) {
                    return config;
                }
            }
        }
        Config::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_config_when_no_file() {
        let tmp = TempDir::new().unwrap();
        let config = Config::load(tmp.path());
        assert!(config.severity_threshold.is_none());
        assert!(config.ignore.is_none());
    }

    #[test]
    fn test_load_config_from_file() {
        let tmp = TempDir::new().unwrap();
        let yaml = "severity_threshold: high\nignore:\n  rules:\n    - DOC-003\n";
        fs::write(tmp.path().join(".repodoctor.yml"), yaml).unwrap();
        let config = Config::load(tmp.path());
        assert_eq!(config.severity_threshold, Some("high".to_string()));
        let ignore = config.ignore.unwrap();
        assert_eq!(ignore.rules.unwrap(), vec!["DOC-003".to_string()]);
    }
}
