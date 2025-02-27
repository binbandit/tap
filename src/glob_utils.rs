use std::path::PathBuf;
use anyhow::Result;
use glob::glob;

/// Expands glob patterns in the provided paths to a list of actual file paths.
/// If a pattern doesn't match any files, it's treated as a path to be created.
/// If a pattern is invalid, it's added as a literal path.
///
/// # Arguments
///
/// * `paths` - A slice of file path strings, which may include glob patterns
///
/// # Returns
///
/// A Result containing a Vec of expanded PathBuf objects
///
/// # Examples
///
/// ```
/// use tap::glob_utils::expand_paths;
///
/// let paths = vec!["src/*.rs".to_string(), "README.md".to_string()];
/// let expanded = expand_paths(&paths).unwrap();
/// // expanded now contains paths to all Rust files in the src directory,
/// // plus the README.md file
/// ```
pub fn expand_paths(paths: &[String]) -> Result<Vec<PathBuf>> {
    let mut expanded = Vec::new();

    for path in paths {
        match glob(path) {
            Ok(entries) => {
                // Collect all entries in one pass
                let mut found_entries = false;
                
                for entry in entries {
                    found_entries = true;
                    match entry {
                        Ok(path) => expanded.push(path),
                        Err(e) => println!("Error processing path {}: {:?}", path, e),
                    }
                }
                
                // If no matches were found, treat it as a new file/directory to be created
                if !found_entries {
                    expanded.push(PathBuf::from(path));
                }
            }
            Err(e) => {
                println!("Invalid glob pattern '{}': {:?}", path, e);
                // Add the path as-is if it's not a valid glob pattern
                expanded.push(PathBuf::from(path));
            }
        }
    }

    Ok(expanded)
} 