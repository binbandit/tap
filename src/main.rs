use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use clap::Parser;
use glob::glob;

#[derive(Parser)]
#[command(name = "tap")]
#[command(about = "A next-gen version of touch with extended capabilities", long_about = None)]
struct Cli {
    /// File(s) or directory to create or update (supports glob patterns)
    #[arg(required = true)]
    paths: Vec<String>,

    /// Create a directory instead of a file
    #[arg(short, long)]
    dir: bool,

    /// Set specific permissions (octal format, e.g., 644)
    #[arg(short, long)]
    chmod: Option<String>,

    /// Add content to the file
    #[arg(short, long)]
    write: Option<String>,

    /// Set access and modification times (format: YYYY-MM-DD HH:MM:SS)
    #[arg(short, long)]
    timestamp: Option<String>,

    /// Append content instead of overwriting
    #[arg(short, long)]
    append: bool,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Apply chmod recursively (only works with directories)
    #[arg(short = 'R', long)]
    recursive: bool,

    /// Use a template file for content
    #[arg(long)]
    template: Option<String>,

    /// Remove trailing whitespace from each line
    #[arg(long)]
    trim: bool,

    /// Check if the file or directory exists (dry run)
    #[arg(long)]
    check: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(&cli)
}

fn run(cli: &Cli) -> Result<()> {
    let expanded_paths = expand_paths(&cli.paths)?;

    for path in expanded_paths {
        if cli.verbose {
            println!("Processing: {}", path.display());
        }

        if cli.check {
            check_existence(&path, cli.verbose)?;
            continue;
        }

        // Ensure parent directories exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create parent directories")?;
        }

        if cli.dir {
            create_directory(&path, cli.verbose)?;
        } else {
            create_or_update_file(&path, cli)?;
        }

        if let Some(chmod) = &cli.chmod {
            set_permissions(&path, chmod, cli.recursive, cli.verbose)?;
        }

        if let Some(timestamp) = &cli.timestamp {
            set_timestamp(&path, timestamp, cli.verbose)?;
        }
    }

    Ok(())
}

fn expand_paths(paths: &[String]) -> Result<Vec<PathBuf>> {
    let mut expanded = Vec::new();

    for path in paths {
        match glob(path) {
            Ok(entries) => {
                let count = entries.count();
                if count == 0 {
                    // If no matches found, treat it as a new file/directory
                    expanded.push(PathBuf::from(path));
                } else {
                    for entry in glob(path).expect("Failed to read glob pattern") {
                        match entry {
                            Ok(path) => expanded.push(path),
                            Err(e) => println!("Error: {:?}", e),
                        }
                    }
                }
            }
            Err(e) => println!("Invalid glob pattern '{}': {:?}", path, e),
        }
    }

    Ok(expanded)
}

fn check_existence(path: &Path, verbose: bool) -> Result<()> {
    if path.exists() {
        if verbose {
            println!("Exists: {}", path.display());
        }
    } else {
        println!("Does not exist: {}", path.display());
    }
    Ok(())
}

fn create_directory(path: &Path, verbose: bool) -> Result<()> {
    fs::create_dir_all(path).context("Failed to create directory")?;
    if verbose {
        println!("Directory created: {}", path.display());
    }
    Ok(())
}

fn create_or_update_file(path: &Path, cli: &Cli) -> Result<()> {
    if cli.trim {
        let content = fs::read_to_string(path).context("Failed to read file content")?;
        let trimmed_content = content
            .lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, trimmed_content).context("Failed to write trimmed content to file")?;
        if cli.verbose {
            println!("Trailing whitespace removed from: {}", path.display());
        }
        return Ok(());
    }

    let mut options = OpenOptions::new();
    options.write(true).create(true);

    if cli.append {
        options.append(true);
    } else if cli.write.is_some() || cli.template.is_some() {
        options.truncate(true);
    }

    let mut file = options
        .open(path)
        .context("Failed to create or open file")?;

    if let Some(template) = &cli.template {
        let content = fs::read_to_string(template).context("Failed to read template file")?;
        file.write_all(content.as_bytes())
            .context("Failed to write template content to file")?;
        if cli.verbose {
            println!(
                "File created/updated with template content: {}",
                path.display()
            );
        }
    } else if let Some(content) = &cli.write {
        file.write_all(content.as_bytes())
            .context("Failed to write content to file")?;
        if cli.verbose {
            if cli.append {
                println!("Content appended to file: {}", path.display());
            } else {
                println!("File created/updated with content: {}", path.display());
            }
        }
    } else if cli.verbose {
        let metadata = file.metadata().context("Failed to get file metadata")?;
        if metadata.len() == 0 {
            println!("File created: {}", path.display());
        } else {
            println!("File timestamp updated: {}", path.display());
        }
    }

    Ok(())
}
fn set_permissions(path: &Path, chmod: &str, recursive: bool, verbose: bool) -> Result<()> {
    let permissions = u32::from_str_radix(chmod, 8).context("Invalid chmod value")?;
    let permissions = fs::Permissions::from_mode(permissions);

    if recursive && path.is_dir() {
        for entry in fs::read_dir(path).context("Failed to read directory")? {
            let entry = entry.context("Failed to read directory entry")?;
            set_permissions(&entry.path(), chmod, recursive, verbose)?;
        }
    }

    fs::set_permissions(path, permissions).context("Failed to set permissions")?;
    if verbose {
        println!("Permissions set to {} for: {}", chmod, path.display());
    }
    Ok(())
}

fn set_timestamp(path: &Path, time_str: &str, verbose: bool) -> Result<()> {
    let timestamp = parse_timestamp(time_str)?;
    let file_time = filetime::FileTime::from_system_time(timestamp);
    filetime::set_file_mtime(path, file_time).context("Failed to set timestamp")?;
    if verbose {
        println!("Timestamp set to {} for: {}", time_str, path.display());
    }
    Ok(())
}

fn parse_timestamp(time_str: &str) -> Result<SystemTime> {
    let dt = NaiveDateTime::parse_from_str(time_str, "%Y-%m-%d %H:%M:%S")
        .context("Invalid timestamp format")?;
    let timestamp =
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(dt.and_utc().timestamp() as u64);
    Ok(timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::{tempdir, NamedTempFile};

    #[test]
    fn test_expand_paths() -> Result<()> {
        let dir = tempdir()?;
        let file1 = dir.path().join("test1.txt");
        let file2 = dir.path().join("test2.txt");
        File::create(&file1)?;
        File::create(&file2)?;

        let paths = vec![dir.path().join("test*.txt").to_string_lossy().to_string()];
        let expanded = expand_paths(&paths)?;

        assert_eq!(expanded.len(), 2);
        assert!(expanded.contains(&file1));
        assert!(expanded.contains(&file2));

        Ok(())
    }

    #[test]
    fn test_check_existence() -> Result<()> {
        let dir = tempdir()?;
        let existing_file = dir.path().join("existing.txt");
        File::create(&existing_file)?;
        let non_existing_file = dir.path().join("non_existing.txt");

        check_existence(&existing_file, false)?;
        check_existence(&non_existing_file, false)?;

        Ok(())
    }

    #[test]
    fn test_create_directory() -> Result<()> {
        let dir = tempdir()?;
        let new_dir = dir.path().join("new_dir");

        create_directory(&new_dir, false)?;

        assert!(new_dir.is_dir());
        Ok(())
    }

    #[test]
    fn test_create_or_update_file() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.txt");

        let cli = Cli {
            paths: vec![file_path.to_string_lossy().to_string()],
            dir: false,
            chmod: None,
            write: Some("Hello, World!".to_string()),
            timestamp: None,
            append: false,
            verbose: false,
            recursive: false,
            template: None,
            trim: false,
            check: false,
        };

        create_or_update_file(&file_path, &cli)?;

        let content = fs::read_to_string(&file_path)?;
        assert_eq!(content, "Hello, World!");

        Ok(())
    }

    #[test]
    fn test_append_to_file() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "Initial content\n")?;

        let cli = Cli {
            paths: vec![file_path.to_string_lossy().to_string()],
            dir: false,
            chmod: None,
            write: Some("Appended content".to_string()),
            timestamp: None,
            append: true,
            verbose: false,
            recursive: false,
            template: None,
            trim: false,
            check: false,
        };

        create_or_update_file(&file_path, &cli)?;

        let content = fs::read_to_string(&file_path)?;
        assert_eq!(content, "Initial content\nAppended content");

        Ok(())
    }

    #[test]
    fn test_trim_whitespace() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "  Line with spaces  \nAnother line \t ")?;

        let cli = Cli {
            paths: vec![file_path.to_string_lossy().to_string()],
            dir: false,
            chmod: None,
            write: None,
            timestamp: None,
            append: false,
            verbose: false,
            recursive: false,
            template: None,
            trim: true,
            check: false,
        };

        create_or_update_file(&file_path, &cli)?;

        let content = fs::read_to_string(&file_path)?;
        assert_eq!(content, "  Line with spaces\nAnother line");

        Ok(())
    }

    #[test]
    fn test_set_permissions() -> Result<()> {
        let file = NamedTempFile::new()?;
        let file_path = file.path();

        set_permissions(file_path, "644", false, false)?;

        let metadata = fs::metadata(file_path)?;
        assert_eq!(metadata.permissions().mode() & 0o777, 0o644);

        Ok(())
    }

    #[test]
    fn test_set_timestamp() -> Result<()> {
        let file = NamedTempFile::new()?;
        let file_path = file.path();

        let time_str = "2023-05-01 12:00:00";
        set_timestamp(file_path, time_str, false)?;

        let metadata = fs::metadata(file_path)?;
        let mtime = metadata.modified()?;
        let expected_time = parse_timestamp(time_str)?;

        assert_eq!(mtime, expected_time);

        Ok(())
    }

    #[test]
    fn test_parse_timestamp() -> Result<()> {
        let time_str = "2023-05-01 12:00:00";
        let parsed_time = parse_timestamp(time_str)?;

        let expected_time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1682942400);
        assert_eq!(parsed_time, expected_time);

        Ok(())
    }

    #[test]
    fn test_run() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.txt");

        let cli = Cli {
            paths: vec![file_path.to_string_lossy().to_string()],
            dir: false,
            chmod: Some("644".to_string()),
            write: Some("Test content".to_string()),
            timestamp: Some("2023-05-01 12:00:00".to_string()),
            append: false,
            verbose: true,
            recursive: false,
            template: None,
            trim: false,
            check: false,
        };

        run(&cli)?;

        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path)?;
        assert_eq!(content, "Test content");

        let metadata = fs::metadata(&file_path)?;
        assert_eq!(metadata.permissions().mode() & 0o777, 0o644);

        let mtime = metadata.modified()?;
        let expected_time = parse_timestamp("2023-05-01 12:00:00")?;
        assert_eq!(mtime, expected_time);

        Ok(())
    }
}
