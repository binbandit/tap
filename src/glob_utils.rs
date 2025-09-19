use anyhow::Result;
use glob::glob;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ExpandedPath {
    pub path: PathBuf,
    pub warnings: Vec<String>,
}
pub fn expand_paths(paths: &[String]) -> Result<Vec<ExpandedPath>> {
    let mut expanded = Vec::new();

    for raw in paths {
        match glob(raw) {
            Ok(entries) => {
                let mut matched = false;

                for entry in entries {
                    match entry {
                        Ok(path) => {
                            matched = true;
                            expanded.push(ExpandedPath {
                                path,
                                warnings: Vec::new(),
                            });
                        }
                        Err(e) => {
                            expanded.push(ExpandedPath {
                                path: PathBuf::from(raw),
                                warnings: vec![format!("Failed to expand '{}': {}", raw, e)],
                            });
                        }
                    }
                }

                if !matched {
                    expanded.push(ExpandedPath {
                        path: PathBuf::from(raw),
                        warnings: vec![format!(
                            "Pattern '{}' matched no files — treating as literal path.",
                            raw
                        )],
                    });
                }
            }
            Err(e) => {
                expanded.push(ExpandedPath {
                    path: PathBuf::from(raw),
                    warnings: vec![format!("Invalid glob pattern '{}': {}", raw, e)],
                });
            }
        }
    }

    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn expands_matching_glob_without_warnings() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "ok").unwrap();

        let pattern = format!("{}/**/*.txt", dir.path().display());
        let results = expand_paths(&[pattern]).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].warnings.is_empty());
        assert_eq!(results[0].path, file);
    }

    #[test]
    fn unmatched_pattern_returns_warning() {
        let dir = tempdir().unwrap();
        let pattern = format!("{}/**/*.md", dir.path().display());
        let results = expand_paths(&[pattern.clone()]).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, PathBuf::from(pattern));
        assert_eq!(results[0].warnings.len(), 1);
    }

    #[test]
    fn invalid_pattern_is_reported() {
        let results = expand_paths(&["[".to_string()]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, PathBuf::from("["));
        assert_eq!(results[0].warnings.len(), 1);
        assert!(results[0].warnings[0].contains("Invalid glob pattern"));
    }
}
