use anyhow::Result;
use clap::Args;
use colored::Colorize;
use std::path::PathBuf;

use crate::frameworks::detector::{Framework, FrameworkDetector};

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Path to the project (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Overwrite existing .repodoctor.yml
    #[arg(long)]
    pub force: bool,
}

pub async fn execute(args: &InitArgs) -> Result<()> {
    let path = args.path.canonicalize()?;
    let config_path = path.join(".repodoctor.yml");

    if config_path.exists() && !args.force {
        println!(
            "  {} .repodoctor.yml already exists. Use {} to overwrite.",
            "SKIP".yellow(),
            "--force".bold()
        );
        return Ok(());
    }

    let detected = FrameworkDetector::detect(&path);
    let config = generate_config(&detected.framework);

    std::fs::write(&config_path, config)?;
    println!(
        "  {} .repodoctor.yml created for {} project",
        "DONE".green(),
        detected.framework.to_string().cyan()
    );
    println!(
        "  Edit {} to customize rules and thresholds.",
        config_path.display()
    );

    Ok(())
}

fn generate_config(framework: &Framework) -> String {
    let ignore_paths = match framework {
        Framework::Symfony | Framework::Laravel => "    - vendor/\n    - var/\n    - node_modules/",
        Framework::Flutter => "    - build/\n    - .dart_tool/\n    - .flutter-plugins",
        Framework::NextJs | Framework::NodeJs => "    - node_modules/\n    - .next/\n    - dist/",
        Framework::RustCargo => "    - target/",
        Framework::Python => "    - __pycache__/\n    - .venv/\n    - dist/",
        Framework::Unknown => "    - node_modules/\n    - vendor/",
    };

    format!(
        r#"# RepoDoctor configuration
# Docs: https://github.com/Jonathanlight/repodoctor

# Minimum severity to report (info, low, medium, high, critical)
severity_threshold: low

# Files and rules to ignore
ignore:
  paths:
{ignore_paths}
  rules: []
    # - DOC-003  # Example: skip CHANGELOG check
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_init_creates_config_file() {
        let tmp = TempDir::new().unwrap();
        let args = InitArgs {
            path: tmp.path().to_path_buf(),
            force: false,
        };
        execute(&args).await.unwrap();
        assert!(tmp.path().join(".repodoctor.yml").exists());
    }

    #[tokio::test]
    async fn test_init_skips_existing_without_force() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".repodoctor.yml"), "existing").unwrap();
        let args = InitArgs {
            path: tmp.path().to_path_buf(),
            force: false,
        };
        execute(&args).await.unwrap();
        let content = fs::read_to_string(tmp.path().join(".repodoctor.yml")).unwrap();
        assert_eq!(content, "existing");
    }

    #[tokio::test]
    async fn test_init_overwrites_with_force() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".repodoctor.yml"), "old").unwrap();
        let args = InitArgs {
            path: tmp.path().to_path_buf(),
            force: true,
        };
        execute(&args).await.unwrap();
        let content = fs::read_to_string(tmp.path().join(".repodoctor.yml")).unwrap();
        assert!(content.contains("severity_threshold"));
    }

    #[tokio::test]
    async fn test_init_detects_framework_config() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nversion = \"0.1.0\"\n").unwrap();
        let args = InitArgs {
            path: tmp.path().to_path_buf(),
            force: false,
        };
        execute(&args).await.unwrap();
        let content = fs::read_to_string(tmp.path().join(".repodoctor.yml")).unwrap();
        assert!(content.contains("target/"));
    }

    #[test]
    fn test_generate_config_symfony() {
        let config = generate_config(&Framework::Symfony);
        assert!(config.contains("vendor/"));
        assert!(config.contains("var/"));
    }

    #[test]
    fn test_generate_config_flutter() {
        let config = generate_config(&Framework::Flutter);
        assert!(config.contains("build/"));
        assert!(config.contains(".dart_tool/"));
    }

    #[test]
    fn test_generate_config_nextjs() {
        let config = generate_config(&Framework::NextJs);
        assert!(config.contains("node_modules/"));
        assert!(config.contains(".next/"));
    }
}
