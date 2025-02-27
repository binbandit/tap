use std::fs;
use std::path::Path;
use anyhow::{anyhow, Context, Result};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Sets the permissions of a file or directory.
///
/// # Arguments
///
/// * `path` - The path to the file or directory
/// * `chmod` - The permissions in octal format (e.g., "644")
/// * `recursive` - Whether to apply permissions recursively (for directories)
/// * `verbose` - Whether to print verbose output
///
/// # Returns
///
/// A Result that contains nothing on success
///
/// # Platform-specific behavior
///
/// On Unix systems, this function sets the full octal permissions.
/// On non-Unix systems, it can only set the read-only flag based on the write bit.
pub fn set_permissions(path: &Path, chmod: &str, recursive: bool, verbose: bool) -> Result<()> {
    // Validate the chmod value is in a reasonable range (000-777)
    let permissions_value = u32::from_str_radix(chmod, 8)
        .context("Invalid chmod value")?;
    
    if permissions_value > 0o777 {
        return Err(anyhow!("Invalid permission value: {}. Must be between 000 and 777", chmod));
    }

    #[cfg(unix)]
    {
        let permissions = fs::Permissions::from_mode(permissions_value);

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
    }
    
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(path)?.permissions();
        
        // On non-Unix systems, we can only set read-only flag
        let readonly = permissions_value & 0o222 == 0; // No write permission
        permissions.set_readonly(readonly);
        
        if recursive && path.is_dir() {
            for entry in fs::read_dir(path).context("Failed to read directory")? {
                let entry = entry.context("Failed to read directory entry")?;
                set_permissions(&entry.path(), chmod, recursive, verbose)?;
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
    }
    
    Ok(())
} 