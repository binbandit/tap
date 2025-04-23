mod cli;
mod file_ops;
mod permissions;
mod timestamp;
mod output;
mod glob_utils;

use std::fs;

use anyhow::Result;
use clap::Parser;

use cli::Cli;
use output::{OutputFormat, OperationResult, format_results};
use glob_utils::expand_paths;
use file_ops::{check_existence, create_directory, create_or_update_file};
use permissions::set_permissions;
use timestamp::set_timestamp;

/// Main entry point for the tap application
fn main() -> Result<()> {
    let cli = Cli::parse();
    run(&cli)
}

/// Core application logic
/// 
/// Processes the provided CLI arguments and performs the requested operations
///
/// # Arguments
///
/// * `cli` - The parsed command-line arguments
///
/// # Returns
///
/// A Result that indicates success or failure
fn run(cli: &Cli) -> Result<()> {
    let expanded_paths = expand_paths(&cli.paths)?;
    let mut results = Vec::new();

    for path in expanded_paths {
        if cli.verbose && cli.output_format == OutputFormat::Text {
            println!("Processing: {}", path.display());
        }

        let mut result = OperationResult {
            path: path.display().to_string(),
            exists: path.exists(),
            is_file: path.is_file(),
            is_dir: path.is_dir(),
            operation: "unknown".to_string(),
            success: false,
            permissions: None,
            timestamp: None,
            message: None,
            error: None,
        };

        if cli.check {
            result.operation = "check".to_string();
            if let Err(e) = check_existence(&path, cli.verbose && cli.output_format == OutputFormat::Text) {
                result.error = Some(e.to_string());
            } else {
                result.success = true;
                result.message = Some(if path.exists() { "Exists" } else { "Does not exist" }.to_string());
            }
            results.push(result);
            continue;
        }

        // Intuitive: auto-detect directory creation if path ends with separator or exists as dir
        let is_dir_path = cli.dir
            || path.is_dir()
            || path.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR)
            || (path.extension().is_none() && !path.exists());

        // Always ensure parent directories exist, just like touch
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                result.operation = "create_parent_directories".to_string();
                result.error = Some(format!("Failed to create parent directories: {}", e));
                results.push(result);
                continue;
            }
        }

        if is_dir_path {
            result.operation = "create_directory".to_string();
            if let Err(e) = create_directory(&path, cli.verbose && cli.output_format == OutputFormat::Text) {
                result.error = Some(e.to_string());
                results.push(result);
                continue;
            }
            result.success = true;
        } else {
            result.operation = "create_or_update_file".to_string();
            if let Err(e) = create_or_update_file(&path, cli) {
                result.error = Some(e.to_string());
                results.push(result);
                continue;
            }
            result.success = true;
        }

        if let Some(chmod) = &cli.chmod {
            result.permissions = Some(chmod.clone());
            if let Err(e) = set_permissions(&path, chmod, cli.recursive, cli.verbose && cli.output_format == OutputFormat::Text) {
                result.error = Some(format!("Failed to set permissions: {}", e));
                result.success = false;
            }
        }

        if let Some(timestamp) = &cli.timestamp {
            result.timestamp = Some(timestamp.clone());
            if let Err(e) = set_timestamp(&path, timestamp, cli.timestamp_format.as_deref(), cli.verbose && cli.output_format == OutputFormat::Text) {
                result.error = Some(format!("Failed to set timestamp: {}", e));
                result.success = false;
            }
        }

        results.push(result);
    }

    // Output the results in the specified format
    match cli.output_format {
        OutputFormat::Text => {
            // Text output is handled by individual functions
        },
        OutputFormat::Json => {
            let json = format_results(&results, OutputFormat::Json)?;
            println!("{}", json);
        },
        OutputFormat::Yaml => {
            let yaml = format_results(&results, OutputFormat::Yaml)?;
            println!("{}", yaml);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::{tempdir, NamedTempFile};
    
    // Include tests here or move them to their respective modules
}
