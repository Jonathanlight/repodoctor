use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::utils::fs::{self, CIProvider};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Framework {
    Symfony,
    Laravel,
    Flutter,
    NextJs,
    RustCargo,
    NodeJs,
    Python,
    Unknown,
}

impl std::fmt::Display for Framework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Framework::Symfony => write!(f, "Symfony"),
            Framework::Laravel => write!(f, "Laravel"),
            Framework::Flutter => write!(f, "Flutter"),
            Framework::NextJs => write!(f, "Next.js"),
            Framework::RustCargo => write!(f, "Rust/Cargo"),
            Framework::NodeJs => write!(f, "Node.js"),
            Framework::Python => write!(f, "Python"),
            Framework::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Php,
    Dart,
    JavaScript,
    TypeScript,
    Python,
    Unknown,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => write!(f, "Rust"),
            Language::Php => write!(f, "PHP"),
            Language::Dart => write!(f, "Dart"),
            Language::JavaScript => write!(f, "JavaScript"),
            Language::TypeScript => write!(f, "TypeScript"),
            Language::Python => write!(f, "Python"),
            Language::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PackageManager {
    Cargo,
    Composer,
    Npm,
    Yarn,
    Pnpm,
    Pip,
    Poetry,
    Pub,
}

impl std::fmt::Display for PackageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageManager::Cargo => write!(f, "Cargo"),
            PackageManager::Composer => write!(f, "Composer"),
            PackageManager::Npm => write!(f, "npm"),
            PackageManager::Yarn => write!(f, "Yarn"),
            PackageManager::Pnpm => write!(f, "pnpm"),
            PackageManager::Pip => write!(f, "pip"),
            PackageManager::Poetry => write!(f, "Poetry"),
            PackageManager::Pub => write!(f, "pub"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedProject {
    pub framework: Framework,
    pub language: Language,
    pub version: Option<String>,
    pub package_manager: Option<PackageManager>,
    pub has_git: bool,
    pub has_ci: Option<CIProvider>,
}

pub struct FrameworkDetector;

impl FrameworkDetector {
    pub fn detect(path: &Path) -> DetectedProject {
        let has_git = fs::has_git_repo(path);
        let has_ci = fs::detect_ci_provider(path);

        // Priority-ordered detection: most specific first
        let indicators: Vec<(&str, Framework, Language, Option<PackageManager>)> = vec![
            ("symfony.lock", Framework::Symfony, Language::Php, Some(PackageManager::Composer)),
            ("config/bundles.php", Framework::Symfony, Language::Php, Some(PackageManager::Composer)),
            ("artisan", Framework::Laravel, Language::Php, Some(PackageManager::Composer)),
            ("pubspec.yaml", Framework::Flutter, Language::Dart, Some(PackageManager::Pub)),
            ("next.config.js", Framework::NextJs, Language::JavaScript, None),
            ("next.config.mjs", Framework::NextJs, Language::JavaScript, None),
            ("next.config.ts", Framework::NextJs, Language::TypeScript, None),
            ("Cargo.toml", Framework::RustCargo, Language::Rust, Some(PackageManager::Cargo)),
            ("package.json", Framework::NodeJs, Language::JavaScript, None),
            ("pyproject.toml", Framework::Python, Language::Python, Some(PackageManager::Poetry)),
            ("requirements.txt", Framework::Python, Language::Python, Some(PackageManager::Pip)),
        ];

        for (file, framework, language, pkg_mgr) in &indicators {
            if fs::path_exists(path, file) {
                let version = Self::detect_version(path, framework);
                let package_manager = pkg_mgr.clone().or_else(|| Self::detect_package_manager(path));

                return DetectedProject {
                    framework: framework.clone(),
                    language: language.clone(),
                    version,
                    package_manager,
                    has_git,
                    has_ci,
                };
            }
        }

        DetectedProject {
            framework: Framework::Unknown,
            language: Language::Unknown,
            version: None,
            package_manager: None,
            has_git,
            has_ci,
        }
    }

    fn detect_version(path: &Path, framework: &Framework) -> Option<String> {
        match framework {
            Framework::RustCargo => Self::version_from_cargo_toml(path),
            Framework::NodeJs | Framework::NextJs => Self::version_from_package_json(path),
            Framework::Flutter => Self::version_from_pubspec(path),
            _ => None,
        }
    }

    fn version_from_cargo_toml(path: &Path) -> Option<String> {
        let content = std::fs::read_to_string(path.join("Cargo.toml")).ok()?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("version") {
                if let Some(val) = trimmed.split('=').nth(1) {
                    return Some(val.trim().trim_matches('"').to_string());
                }
            }
        }
        None
    }

    fn version_from_package_json(path: &Path) -> Option<String> {
        let content = std::fs::read_to_string(path.join("package.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        json.get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn version_from_pubspec(path: &Path) -> Option<String> {
        let content = std::fs::read_to_string(path.join("pubspec.yaml")).ok()?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("version:") {
                return Some(trimmed.trim_start_matches("version:").trim().to_string());
            }
        }
        None
    }

    fn detect_package_manager(path: &Path) -> Option<PackageManager> {
        if path.join("yarn.lock").exists() {
            Some(PackageManager::Yarn)
        } else if path.join("pnpm-lock.yaml").exists() {
            Some(PackageManager::Pnpm)
        } else if path.join("package-lock.json").exists() {
            Some(PackageManager::Npm)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs as stdfs;
    use tempfile::TempDir;

    fn setup_tmp() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_detect_unknown_empty_dir() {
        let tmp = setup_tmp();
        let detected = FrameworkDetector::detect(tmp.path());
        assert_eq!(detected.framework, Framework::Unknown);
        assert_eq!(detected.language, Language::Unknown);
    }

    #[test]
    fn test_detect_rust_cargo() {
        let tmp = setup_tmp();
        stdfs::write(tmp.path().join("Cargo.toml"), "[package]\nversion = \"0.1.0\"\n").unwrap();
        let detected = FrameworkDetector::detect(tmp.path());
        assert_eq!(detected.framework, Framework::RustCargo);
        assert_eq!(detected.language, Language::Rust);
        assert_eq!(detected.version, Some("0.1.0".to_string()));
        assert_eq!(detected.package_manager, Some(PackageManager::Cargo));
    }

    #[test]
    fn test_detect_nodejs() {
        let tmp = setup_tmp();
        stdfs::write(tmp.path().join("package.json"), r#"{"version": "1.2.3"}"#).unwrap();
        let detected = FrameworkDetector::detect(tmp.path());
        assert_eq!(detected.framework, Framework::NodeJs);
        assert_eq!(detected.language, Language::JavaScript);
        assert_eq!(detected.version, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_detect_nextjs_over_nodejs() {
        let tmp = setup_tmp();
        stdfs::write(tmp.path().join("package.json"), r#"{"version": "2.0.0"}"#).unwrap();
        stdfs::write(tmp.path().join("next.config.js"), "module.exports = {}").unwrap();
        let detected = FrameworkDetector::detect(tmp.path());
        assert_eq!(detected.framework, Framework::NextJs);
    }

    #[test]
    fn test_detect_flutter() {
        let tmp = setup_tmp();
        stdfs::write(tmp.path().join("pubspec.yaml"), "name: myapp\nversion: 1.0.0+1\n").unwrap();
        let detected = FrameworkDetector::detect(tmp.path());
        assert_eq!(detected.framework, Framework::Flutter);
        assert_eq!(detected.language, Language::Dart);
        assert_eq!(detected.version, Some("1.0.0+1".to_string()));
        assert_eq!(detected.package_manager, Some(PackageManager::Pub));
    }

    #[test]
    fn test_detect_symfony() {
        let tmp = setup_tmp();
        stdfs::write(tmp.path().join("symfony.lock"), "{}").unwrap();
        let detected = FrameworkDetector::detect(tmp.path());
        assert_eq!(detected.framework, Framework::Symfony);
        assert_eq!(detected.language, Language::Php);
    }

    #[test]
    fn test_detect_laravel() {
        let tmp = setup_tmp();
        stdfs::write(tmp.path().join("artisan"), "#!/usr/bin/env php").unwrap();
        let detected = FrameworkDetector::detect(tmp.path());
        assert_eq!(detected.framework, Framework::Laravel);
        assert_eq!(detected.language, Language::Php);
    }

    #[test]
    fn test_detect_python_pyproject() {
        let tmp = setup_tmp();
        stdfs::write(tmp.path().join("pyproject.toml"), "[tool.poetry]").unwrap();
        let detected = FrameworkDetector::detect(tmp.path());
        assert_eq!(detected.framework, Framework::Python);
        assert_eq!(detected.language, Language::Python);
        assert_eq!(detected.package_manager, Some(PackageManager::Poetry));
    }

    #[test]
    fn test_detect_python_requirements() {
        let tmp = setup_tmp();
        stdfs::write(tmp.path().join("requirements.txt"), "flask==2.0").unwrap();
        let detected = FrameworkDetector::detect(tmp.path());
        assert_eq!(detected.framework, Framework::Python);
        assert_eq!(detected.package_manager, Some(PackageManager::Pip));
    }

    #[test]
    fn test_detect_git_repo() {
        let tmp = setup_tmp();
        stdfs::create_dir(tmp.path().join(".git")).unwrap();
        let detected = FrameworkDetector::detect(tmp.path());
        assert!(detected.has_git);
    }

    #[test]
    fn test_detect_ci_github() {
        let tmp = setup_tmp();
        stdfs::create_dir_all(tmp.path().join(".github/workflows")).unwrap();
        let detected = FrameworkDetector::detect(tmp.path());
        assert_eq!(detected.has_ci, Some(CIProvider::GitHubActions));
    }

    #[test]
    fn test_detect_package_manager_yarn() {
        let tmp = setup_tmp();
        stdfs::write(tmp.path().join("package.json"), r#"{"version": "1.0.0"}"#).unwrap();
        stdfs::write(tmp.path().join("yarn.lock"), "").unwrap();
        let detected = FrameworkDetector::detect(tmp.path());
        assert_eq!(detected.package_manager, Some(PackageManager::Yarn));
    }
}
