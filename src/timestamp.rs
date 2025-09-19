use std::path::Path;
use std::time::SystemTime;

use crate::output::OperationResult;
use anyhow::{anyhow, Context, Result};
use chrono::NaiveDateTime;
use filetime::FileTime;

pub fn set_timestamp(
    path: &Path,
    time_str: &str,
    format: Option<&str>,
    verbose: bool,
    result: &mut OperationResult,
) -> Result<()> {
    let timestamp = parse_timestamp(time_str, format)?;
    let file_time = FileTime::from_system_time(timestamp);
    filetime::set_file_mtime(path, file_time).context("Failed to set timestamp")?;
    if verbose {
        println!("Timestamp set to {} for: {}", time_str, path.display());
    }
    result
        .details
        .push(format!("timestamp set to {}", time_str));
    Ok(())
}

pub fn parse_timestamp(time_str: &str, format: Option<&str>) -> Result<SystemTime> {
    let format = format.unwrap_or("%Y-%m-%d %H:%M:%S");

    match NaiveDateTime::parse_from_str(time_str, format) {
        Ok(dt) => {
            let datetime = dt.and_utc();
            Ok(datetime.into())
        }
        Err(e) => {
            let alternate_formats = [
                "%Y-%m-%d",
                "%Y/%m/%d %H:%M:%S",
                "%Y/%m/%d",
                "%d-%m-%Y %H:%M:%S",
                "%d/%m/%Y %H:%M:%S",
                "%m/%d/%Y %H:%M:%S",
                "%b %d %Y %H:%M:%S",
                "%d %b %Y %H:%M:%S",
            ];

            for alt_format in alternate_formats {
                if let Ok(dt) = NaiveDateTime::parse_from_str(time_str, alt_format) {
                    let datetime = dt.and_utc();
                    return Ok(datetime.into());
                }
            }

            Err(anyhow!("Invalid timestamp format. Error: {}", e))
        }
    }
}
