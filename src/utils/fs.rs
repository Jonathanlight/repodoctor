use serde::{Deserialize, Serialize};
use std::path::Path;
use walkdir::WalkDir;

pub fn path_exists(base: &Path, relative: &str) -> bool {
    base.join(relative).exists()
}

pub fn has_git_repo(path: &Path) -> bool {
    path.join(".git").is_dir()
}

pub fn max_directory_depth(path: &Path) -> usize {
    let mut max_depth = 0;
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.')
                && name != "node_modules"
                && name != "vendor"
                && name != "target"
                && name != "__pycache__"
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() {
            let depth = entry.depth();
            if depth > max_depth {
                max_depth = depth;
            }
        }
    }
    max_depth
}

#[allow(dead_code)]
pub fn find_files_by_name(path: &Path, name: &str) -> Vec<std::path::PathBuf> {
    let mut results = Vec::new();
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let n = e.file_name().to_string_lossy();
            !n.starts_with('.')
                && n != "node_modules"
                && n != "vendor"
                && n != "target"
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.file_name().to_string_lossy() == name {
            results.push(entry.into_path());
        }
    }
    results
}

pub fn find_files_with_extension(path: &Path, ext: &str) -> Vec<std::path::PathBuf> {
    let mut results = Vec::new();
    let dot_ext = if ext.starts_with('.') {
        ext.to_string()
    } else {
        format!(".{}", ext)
    };
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let n = e.file_name().to_string_lossy();
            !n.starts_with('.')
                && n != "node_modules"
                && n != "vendor"
                && n != "target"
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy();
            if name.ends_with(&dot_ext) {
                results.push(entry.into_path());
            }
        }
    }
    results
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CIProvider {
    GitHubActions,
    GitLabCI,
    CircleCI,
    TravisCI,
    JenkinsFile,
}

pub fn detect_ci_provider(path: &Path) -> Option<CIProvider> {
    if path.join(".github/workflows").is_dir() {
        Some(CIProvider::GitHubActions)
    } else if path.join(".gitlab-ci.yml").exists() {
        Some(CIProvider::GitLabCI)
    } else if path.join(".circleci/config.yml").exists() {
        Some(CIProvider::CircleCI)
    } else if path.join(".travis.yml").exists() {
        Some(CIProvider::TravisCI)
    } else if path.join("Jenkinsfile").exists() {
        Some(CIProvider::JenkinsFile)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_path_exists() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("test.txt"), "hello").unwrap();
        assert!(path_exists(tmp.path(), "test.txt"));
        assert!(!path_exists(tmp.path(), "missing.txt"));
    }

    #[test]
    fn test_has_git_repo() {
        let tmp = TempDir::new().unwrap();
        assert!(!has_git_repo(tmp.path()));
        fs::create_dir(tmp.path().join(".git")).unwrap();
        assert!(has_git_repo(tmp.path()));
    }

    #[test]
    fn test_max_directory_depth() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("a/b/c")).unwrap();
        assert_eq!(max_directory_depth(tmp.path()), 3);
    }

    #[test]
    fn test_find_files_by_name() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("README.md"), "# Hi").unwrap();
        fs::write(tmp.path().join("sub/README.md"), "# Sub").unwrap();
        let results = find_files_by_name(tmp.path(), "README.md");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_detect_ci_provider() {
        let tmp = TempDir::new().unwrap();
        assert!(detect_ci_provider(tmp.path()).is_none());

        fs::create_dir_all(tmp.path().join(".github/workflows")).unwrap();
        assert_eq!(detect_ci_provider(tmp.path()), Some(CIProvider::GitHubActions));
    }
}
