use std::path::Path;
use std::time::SystemTime;
use anyhow::{anyhow, Context, Result};
use chrono::NaiveDateTime;
use filetime::FileTime;

/// Sets the modification time of a file.
///
/// # Arguments
///
/// * `path` - The path to the file or directory
/// * `time_str` - The timestamp string to parse
/// * `format` - Optional custom format for the timestamp
/// * `verbose` - Whether to print verbose output
///
/// # Returns
///
/// A Result that contains nothing on success
pub fn set_timestamp(path: &Path, time_str: &str, format: Option<&str>, verbose: bool) -> Result<()> {
    let timestamp = parse_timestamp(time_str, format)?;
    let file_time = FileTime::from_system_time(timestamp);
    filetime::set_file_mtime(path, file_time).context("Failed to set timestamp")?;
    if verbose {
        println!("Timestamp set to {} for: {}", time_str, path.display());
    }
    Ok(())
}

/// Parses a timestamp string into a SystemTime.
///
/// # Arguments
///
/// * `time_str` - The timestamp string to parse
/// * `format` - Optional custom format for the timestamp
///
/// # Returns
///
/// A Result containing the parsed SystemTime
///
/// # Examples
///
/// ```
/// use tap::timestamp::parse_timestamp;
/// use std::time::SystemTime;
///
/// let time = parse_timestamp("2023-01-01 12:00:00", None).unwrap();
/// // time is now set to January 1, 2023 at noon
/// ```
pub fn parse_timestamp(time_str: &str, format: Option<&str>) -> Result<SystemTime> {
    // Default format for backward compatibility
    let format = format.unwrap_or("%Y-%m-%d %H:%M:%S");
    
    // Try parsing with the provided format
    match NaiveDateTime::parse_from_str(time_str, format) {
        Ok(dt) => {
            let timestamp = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(dt.and_utc().timestamp() as u64);
            Ok(timestamp)
        },
        Err(e) => {
            // If parsing fails, try common formats
            let alternate_formats = [
                "%Y-%m-%d",             // ISO date only
                "%Y/%m/%d %H:%M:%S",    // alternate date-time
                "%Y/%m/%d",             // alternate date
                "%d-%m-%Y %H:%M:%S",    // European date-time
                "%d/%m/%Y %H:%M:%S",    // European alternate date-time
                "%m/%d/%Y %H:%M:%S",    // US date-time
                "%b %d %Y %H:%M:%S",    // Month name date-time (Jan 01 2023 12:00:00)
                "%d %b %Y %H:%M:%S",    // European month name (01 Jan 2023 12:00:00)
            ];
            
            for alt_format in alternate_formats {
                if let Ok(dt) = NaiveDateTime::parse_from_str(time_str, alt_format) {
                    let timestamp = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(dt.and_utc().timestamp() as u64);
                    return Ok(timestamp);
                }
            }
            
            // Return the original error if no formats worked
            Err(anyhow!("Invalid timestamp format. Error: {}", e))
        }
    }
} 