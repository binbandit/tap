use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::Path;

use crate::output::OperationResult;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub fn set_permissions(
    path: &Path,
    chmod: &str,
    recursive: bool,
    verbose: bool,
    result: &mut OperationResult,
    is_root: bool,
) -> Result<()> {
    let permissions_value = u32::from_str_radix(chmod, 8).context("Invalid chmod value")?;

    if permissions_value > 0o777 {
        return Err(anyhow!(
            "Invalid permission value: {}. Must be between 000 and 777",
            chmod
        ));
    }

    #[cfg(unix)]
    {
        let permissions = fs::Permissions::from_mode(permissions_value);

        if recursive && path.is_dir() {
            for entry in fs::read_dir(path).context("Failed to read directory")? {
                let entry = entry.context("Failed to read directory entry")?;
                set_permissions(&entry.path(), chmod, recursive, verbose, result, false)?;
            }
        }

        fs::set_permissions(path, permissions).context("Failed to set permissions")?;
        if verbose {
            println!("Permissions set to {} for: {}", chmod, path.display());
        }
        if is_root {
            result.details.push(format!("permissions set to {}", chmod));
        }
    }

    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(path)?.permissions();

        let readonly = (permissions_value & 0o222) == 0;
        permissions.set_readonly(readonly);

        if recursive && path.is_dir() {
            for entry in fs::read_dir(path).context("Failed to read directory")? {
                let entry = entry.context("Failed to read directory entry")?;
                set_permissions(&entry.path(), chmod, recursive, verbose, result, false)?;
            }
        }

        fs::set_permissions(path, permissions).context("Failed to set permissions")?;
        if verbose {
            if readonly {
                println!("Set read-only permissions for: {}", path.display());
            } else {
                println!("Set read-write permissions for: {}", path.display());
            }
            println!("Note: On this platform, only read-only flag can be set (approximated from octal value)");
        }

        if is_root {
            if readonly {
                result.details.push(
                    "permissions approximated as read-only (platform limitation)".to_string(),
                );
            } else {
                result.details.push(
                    "permissions approximated as read-write (platform limitation)".to_string(),
                );
            }
        }
    }

    Ok(())
}
