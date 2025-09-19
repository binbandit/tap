mod cli;
mod file_ops;
mod glob_utils;
mod output;
mod permissions;
mod timestamp;

use std::fs;

use anyhow::Result;
use clap::Parser;

use cli::Cli;
use file_ops::{check_existence, create_directory, create_or_update_file};
use glob_utils::{expand_paths, ExpandedPath};
use output::{format_results, OperationResult, OutputFormat};
use permissions::set_permissions;
use timestamp::set_timestamp;

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(&cli)
}

fn run(cli: &Cli) -> Result<()> {
    let interactive = cli.output_format == OutputFormat::Text && atty::is(atty::Stream::Stdout);
    let expanded_paths = expand_paths(&cli.paths)?;
    let mut results = Vec::new();

    for ExpandedPath { path, warnings } in expanded_paths {
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
            warnings,
            details: Vec::new(),
        };

        if cli.output_format == OutputFormat::Text && (cli.verbose || interactive) {
            for warning in &result.warnings {
                eprintln!("Warning: {}", warning);
            }
        }

        if cli.check {
            result.operation = "check".to_string();
            if let Err(e) = check_existence(
                &path,
                cli.verbose && cli.output_format == OutputFormat::Text,
                interactive,
                &mut result,
            ) {
                result.error = Some(e.to_string());
            } else {
                result.success = true;
            }
            results.push(result);
            continue;
        }

        let is_dir_path =
            cli.dir || path.is_dir() || path.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR);

        if let Some(parent) = path.parent() {
            let parent_is_root = parent.as_os_str().is_empty();
            if !parent_is_root && !parent.exists() {
                if cli.no_parent {
                    result.operation = "create_parent_directories".to_string();
                    result.error = Some(format!(
                        "Parent directory '{}' does not exist. Remove --no-parent to create it automatically.",
                        parent.display()
                    ));
                    results.push(result);
                    continue;
                }

                if let Err(e) = fs::create_dir_all(parent) {
                    result.operation = "create_parent_directories".to_string();
                    result.error = Some(format!("Failed to create parent directories: {}", e));
                    results.push(result);
                    continue;
                }

                if cli.verbose && cli.output_format == OutputFormat::Text {
                    println!("Created missing parent directories for: {}", path.display());
                }

                result
                    .details
                    .push("parent directories created".to_string());
            }
        }

        if path.exists() {
            if is_dir_path && path.is_file() {
                result.operation = "error".to_string();
                result.error = Some(format!(
                    "Cannot create directory '{}': a file already exists at this path.",
                    path.display()
                ));
                results.push(result);
                continue;
            } else if !is_dir_path && path.is_dir() {
                result.operation = "error".to_string();
                result.error = Some(format!(
                    "Cannot create file '{}': a directory already exists at this path.",
                    path.display()
                ));
                results.push(result);
                continue;
            }
        }

        if is_dir_path && (cli.encoding.is_some() || cli.line_endings.is_some()) {
            result
                .warnings
                .push("--encoding and --line-endings are ignored for directories".to_string());
            if cli.verbose || interactive {
                eprintln!("Warning: --encoding and --line-endings have no effect on directories. Skipping.");
            }
        }

        let print_success = cli.verbose;

        if is_dir_path {
            result.operation = "create_directory".to_string();
            if let Err(e) = create_directory(
                &path,
                cli.verbose && cli.output_format == OutputFormat::Text,
                &mut result,
            ) {
                result.error = Some(e.to_string());
                results.push(result);
                continue;
            }
            result.success = true;
            if print_success && cli.output_format == OutputFormat::Text {
                println!("Directory created: {}", path.display());
            }
        } else {
            result.operation = "create_or_update_file".to_string();
            if let Err(e) = create_or_update_file(&path, cli, &mut result) {
                result.error = Some(e.to_string());
                results.push(result);
                continue;
            }
            result.success = true;
            if print_success && cli.output_format == OutputFormat::Text {
                println!("File created or updated: {}", path.display());
            }
        }

        if let Some(chmod) = &cli.chmod {
            result.permissions = Some(chmod.clone());
            if let Err(e) = set_permissions(
                &path,
                chmod,
                cli.recursive,
                cli.verbose && cli.output_format == OutputFormat::Text,
                &mut result,
                true,
            ) {
                result.error = Some(format!("Failed to set permissions: {}", e));
                result.success = false;
            }
        }

        if let Some(timestamp) = &cli.timestamp {
            result.timestamp = Some(timestamp.clone());
            if let Err(e) = set_timestamp(
                &path,
                timestamp,
                cli.timestamp_format.as_deref(),
                cli.verbose && cli.output_format == OutputFormat::Text,
                &mut result,
            ) {
                result.error = Some(format!("Failed to set timestamp: {}", e));
                result.success = false;
            }
        }

        results.push(result);
    }

    match cli.output_format {
        OutputFormat::Text => {}
        OutputFormat::Json => {
            let json = format_results(&results, OutputFormat::Json)?;
            println!("{}", json);
        }
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
    use crate::cli::Cli;
    use tempfile::tempdir;

    fn base_cli(paths: Vec<String>) -> Cli {
        Cli {
            paths,
            dir: false,
            chmod: None,
            write: None,
            timestamp: None,
            append: false,
            verbose: false,
            recursive: false,
            template: None,
            trim: false,
            check: false,
            no_parent: false,
            line_endings: None,
            encoding: None,
            timestamp_format: None,
            output_format: OutputFormat::Text,
        }
    }

    #[test]
    fn plain_name_without_dir_flag_creates_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("README");
        let cli = base_cli(vec![path.to_string_lossy().to_string()]);

        run(&cli).expect("tap run should succeed");

        assert!(path.is_file());
        assert!(!path.is_dir());
    }

    #[test]
    fn missing_parent_is_created_automatically() {
        let dir = tempdir().unwrap();
        let nested_path = dir.path().join("nested/child.txt");
        let nested_string = nested_path.to_string_lossy().to_string();

        let cli = base_cli(vec![nested_string.clone()]);
        run(&cli).expect("run with missing parents should not panic");
        assert!(nested_path.exists());
    }

    #[test]
    fn no_parent_flag_preserves_failure() {
        let dir = tempdir().unwrap();
        let nested_path = dir.path().join("nested/child.txt");
        let nested_string = nested_path.to_string_lossy().to_string();

        let mut cli = base_cli(vec![nested_string]);
        cli.no_parent = true;
        run(&cli).expect("run should complete even when path creation fails");
        assert!(!nested_path.exists());
    }
}
