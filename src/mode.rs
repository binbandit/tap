//! Permission handling: octal modes and the executable bit.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Parse an octal mode string like "644" or "2755".
pub fn parse_mode(s: &str) -> Result<u32> {
    let value = u32::from_str_radix(s, 8)
        .map_err(|_| anyhow!("mode '{s}' is not octal - try something like 644 or 755"))?;
    if s.len() > 4 || value > 0o7777 {
        return Err(anyhow!("mode '{s}' is out of range (000-7777)"));
    }
    Ok(value)
}

/// Apply a mode to a path, optionally recursing into directories.
pub fn apply_mode(path: &Path, mode: u32, recursive: bool) -> Result<()> {
    if recursive && path.is_dir() {
        for entry in fs::read_dir(path)
            .with_context(|| format!("failed to read directory '{}'", path.display()))?
        {
            let entry = entry.context("failed to read directory entry")?;
            apply_mode(&entry.path(), mode, recursive)?;
        }
    }

    set_mode(path, mode)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("failed to set mode on '{}'", path.display()))
}

#[cfg(not(unix))]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    // Windows only models a read-only bit; approximate from the write bits.
    let mut permissions = fs::metadata(path)
        .with_context(|| format!("failed to read metadata for '{}'", path.display()))?
        .permissions();
    permissions.set_readonly((mode & 0o222) == 0);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("failed to set permissions on '{}'", path.display()))
}

/// Add execute bits (u+x, g+x, o+x where read is allowed). Returns false when
/// the platform can't express an executable bit.
pub fn make_executable(path: &Path) -> Result<bool> {
    #[cfg(unix)]
    {
        let metadata = fs::metadata(path)
            .with_context(|| format!("failed to read metadata for '{}'", path.display()))?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(permissions.mode() | 0o111);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to make '{}' executable", path.display()))?;
        Ok(true)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_modes() {
        assert_eq!(parse_mode("644").unwrap(), 0o644);
        assert_eq!(parse_mode("755").unwrap(), 0o755);
        assert_eq!(parse_mode("2755").unwrap(), 0o2755);
    }

    #[test]
    fn rejects_bad_modes_with_guidance() {
        assert!(parse_mode("abc").is_err());
        assert!(parse_mode("99").is_err());
        assert!(parse_mode("77777").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn applies_mode_recursively() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("child.txt");
        std::fs::write(&child, "x").unwrap();

        apply_mode(dir.path(), 0o700, true).unwrap();
        let mode = std::fs::metadata(&child).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[cfg(unix)]
    #[test]
    fn exec_bit_is_added_without_clobbering() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("script.sh");
        std::fs::write(&file, "x").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();

        assert!(make_executable(&file).unwrap());
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o751);
    }
}
