//! The per-path engine: decide what a target needs, then do it.

use std::collections::HashSet;
use std::fs::{self, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::Cli;
use crate::expand::Target;
use crate::mode;
use crate::report::{Action, Kind, PathReport};
use crate::times::{self, RequestedTimes};

/// Everything that is identical across paths, resolved once per invocation.
pub struct Plan {
    /// Prepared content bytes from --write (newline-terminated) or --template.
    pub content: Option<Vec<u8>>,
    pub mode: Option<u32>,
    pub times: Option<RequestedTimes>,
}

/// Directories known to exist, so bulk runs stat each parent once instead of
/// once per file.
#[derive(Default)]
pub struct ParentCache(HashSet<PathBuf>);

impl Plan {
    /// Validate and resolve the shared parts of the invocation. Errors here
    /// are usage errors and abort the whole run before anything is touched.
    pub fn prepare(cli: &Cli) -> Result<Self> {
        let content = if let Some(template) = &cli.template {
            Some(
                fs::read(template)
                    .with_context(|| format!("cannot read template file '{template}'"))?,
            )
        } else {
            cli.write.as_ref().map(|text| {
                let mut bytes = text.clone().into_bytes();
                if !bytes.ends_with(b"\n") {
                    bytes.push(b'\n');
                }
                bytes
            })
        };

        let mode = cli.mode.as_deref().map(mode::parse_mode).transpose()?;
        let times = times::resolve_requested_times(cli)?;

        Ok(Self {
            content,
            mode,
            times,
        })
    }
}

/// Process one target and report what happened. Never panics, never prints.
pub fn process(cli: &Cli, plan: &Plan, target: &Target, parents: &mut ParentCache) -> PathReport {
    let mut report = PathReport::new(target.path.display().to_string());
    report.warnings.extend(target.warnings.iter().cloned());

    let path = &target.path;
    // One stat answers exists/is_dir/is_file/len for the whole run.
    let meta = fs::metadata(path).ok();
    let exists = meta.is_some();
    let is_dir = meta.as_ref().is_some_and(Metadata::is_dir);
    let is_file = meta.as_ref().is_some_and(Metadata::is_file);

    if cli.check {
        report.kind = kind_of(is_dir, is_file);
        report.action = if exists {
            Action::Exists
        } else {
            Action::Missing
        };
        report.ok = exists;
        return report;
    }

    if target.unmatched_pattern {
        report.fail(
            format!("pattern '{}' matched nothing", target.given),
            Some("globs only match existing files; spell the name out to create it"),
        );
        return report;
    }

    let trailing_separator =
        target.given.ends_with('/') || target.given.ends_with(std::path::MAIN_SEPARATOR);
    let want_dir = cli.dir || trailing_separator || is_dir;
    report.kind = Some(if want_dir { Kind::Dir } else { Kind::File });

    if exists && want_dir && is_file {
        report.fail(
            "a file already exists at this path",
            Some("remove the trailing '/' (or -d) to treat it as a file"),
        );
        return report;
    }

    if want_dir {
        let mut ignored = Vec::new();
        if cli.write.is_some() {
            ignored.push("--write");
        }
        if cli.template.is_some() {
            ignored.push("--template");
        }
        if cli.exec {
            ignored.push("--exec");
        }
        if !ignored.is_empty() {
            report
                .warnings
                .push(format!("{} ignored for directories", ignored.join(", ")));
        }
    }

    if cli.no_create && !exists {
        // Like touch -c: not an error, just nothing to do.
        report.action = Action::Skipped;
        report.ok = true;
        return report;
    }

    let file_len = if is_file {
        meta.as_ref().map_or(0, Metadata::len)
    } else {
        0
    };

    match execute(
        cli, plan, target, &mut report, exists, want_dir, file_len, parents,
    ) {
        Ok(()) => report.ok = report.error.is_none(),
        Err(e) => report.fail(format!("{e:#}"), None),
    }
    report
}

#[allow(clippy::too_many_arguments)]
fn execute(
    cli: &Cli,
    plan: &Plan,
    target: &Target,
    report: &mut PathReport,
    existed: bool,
    want_dir: bool,
    file_len: u64,
    parents: &mut ParentCache,
) -> Result<()> {
    let path = &target.path;

    // Parent directories. The cache means N files in one directory check it once.
    let mut needs_parents = false;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parents.0.contains(parent) {
            if parent.exists() {
                parents.0.insert(parent.to_path_buf());
            } else if cli.no_parents {
                report.fail(
                    format!("parent directory '{}' does not exist", parent.display()),
                    Some("drop --no-parents and tap will create it for you"),
                );
                return Ok(());
            } else {
                needs_parents = true;
            }
        }
    }

    // Content safety: refuse to clobber real content without --force.
    if !want_dir && plan.content.is_some() && !cli.append && file_len > 0 && !cli.force {
        report.fail(
            "refusing to overwrite existing content",
            Some("use --append to add to it, or --force to replace it"),
        );
        return Ok(());
    }

    if cli.dry_run {
        preview(cli, plan, report, existed, want_dir, needs_parents);
        return Ok(());
    }

    if needs_parents {
        let parent = path.parent().expect("checked above");
        fs::create_dir_all(parent).context("failed to create parent directories")?;
        parents.0.insert(parent.to_path_buf());
        report.changes.push("parents created".into());
    }

    if want_dir {
        if !existed {
            fs::create_dir_all(path).context("failed to create directory")?;
        }
        parents.0.insert(path.to_path_buf());
    } else if !existed || plan.content.is_some() {
        // An existing file with no content to write only needs its times
        // refreshed below; opening it would be a wasted syscall.
        write_file(cli, plan, path, report, existed, file_len)?;
    }

    // Permissions.
    if cli.exec && !want_dir {
        if mode::make_executable(path)? {
            report.changes.push("made executable".into());
        } else {
            report
                .warnings
                .push("--exec has no effect on this platform".into());
        }
    }
    if let Some(m) = plan.mode {
        mode::apply_mode(path, m, cli.recursive)?;
        report.changes.push(format!(
            "mode {:o}{}",
            m,
            if cli.recursive { " (recursive)" } else { "" }
        ));
    }

    // Times. Brand-new paths already carry "now"; only call out explicit requests.
    if plan.times.is_some() || existed {
        times::apply_times(path, plan.times, cli.atime, cli.mtime)?;
    }
    if plan.times.is_some() {
        report.changes.push("times set".into());
    }

    report.action = if !existed {
        Action::Created
    } else if report.changes.is_empty() || plan.times.is_some() && report.changes == ["times set"] {
        Action::Touched
    } else {
        Action::Updated
    };

    Ok(())
}

fn write_file(
    cli: &Cli,
    plan: &Plan,
    path: &Path,
    report: &mut PathReport,
    existed: bool,
    file_len: u64,
) -> Result<()> {
    match &plan.content {
        Some(bytes) if cli.append => {
            let mut file = OpenOptions::new()
                .read(true)
                .append(true)
                .create(true)
                .open(path)
                .context("failed to open file for appending")?;

            // If the file doesn't end with a newline, add one so the
            // appended content starts on its own line.
            if file_len > 0 {
                let mut last = [0u8; 1];
                file.seek(SeekFrom::End(-1))?;
                file.read_exact(&mut last)?;
                if last[0] != b'\n' {
                    file.write_all(b"\n")?;
                }
            }
            file.write_all(bytes).context("failed to append content")?;
            report.changes.push("content appended".into());
        }
        Some(bytes) => {
            fs::write(path, bytes).context("failed to write content")?;
            report.changes.push(if cli.template.is_some() {
                "template applied".into()
            } else {
                "content written".into()
            });
        }
        None => {
            // A bare touch must never disturb existing content.
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)
                .context("failed to create file")?;

            if !existed && cli.exec {
                if let Some(shebang) = shebang_for(path) {
                    fs::write(path, shebang).context("failed to write shebang")?;
                    report.changes.push("shebang added".into());
                }
            }
        }
    }
    Ok(())
}

fn preview(
    cli: &Cli,
    plan: &Plan,
    report: &mut PathReport,
    existed: bool,
    want_dir: bool,
    needs_parents: bool,
) {
    if needs_parents {
        report.changes.push("parents created".into());
    }
    if plan.content.is_some() && !want_dir {
        report.changes.push(if cli.append {
            "content appended".into()
        } else {
            "content written".into()
        });
    }
    if !existed
        && !want_dir
        && cli.exec
        && plan.content.is_none()
        && shebang_for(&report.path).is_some()
    {
        report.changes.push("shebang added".into());
    }
    if cli.exec && !want_dir {
        report.changes.push("made executable".into());
    }
    if let Some(m) = plan.mode {
        report.changes.push(format!("mode {m:o}"));
    }
    if plan.times.is_some() {
        report.changes.push("times set".into());
    }

    report.action = if !existed {
        Action::WouldCreate
    } else if report.changes.is_empty() {
        Action::WouldTouch
    } else {
        Action::WouldUpdate
    };
}

/// Pick a shebang for a brand-new executable script based on its extension.
/// Extensionless files default to bash - the overwhelmingly common case for
/// `tap -x scripts/deploy`.
fn shebang_for(path: impl AsRef<Path>) -> Option<&'static str> {
    let path = path.as_ref();
    let ext = path.extension().and_then(|e| e.to_str());
    match ext {
        None | Some("sh") | Some("bash") => Some("#!/usr/bin/env bash\n"),
        Some("zsh") => Some("#!/usr/bin/env zsh\n"),
        Some("fish") => Some("#!/usr/bin/env fish\n"),
        Some("py") => Some("#!/usr/bin/env python3\n"),
        Some("rb") => Some("#!/usr/bin/env ruby\n"),
        Some("pl") => Some("#!/usr/bin/env perl\n"),
        Some("js") | Some("mjs") => Some("#!/usr/bin/env node\n"),
        _ => None,
    }
}

fn kind_of(is_dir: bool, is_file: bool) -> Option<Kind> {
    if is_dir {
        Some(Kind::Dir)
    } else if is_file {
        Some(Kind::File)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shebangs_match_extensions() {
        assert_eq!(shebang_for("run.py"), Some("#!/usr/bin/env python3\n"));
        assert_eq!(shebang_for("run.sh"), Some("#!/usr/bin/env bash\n"));
        assert_eq!(shebang_for("deploy"), Some("#!/usr/bin/env bash\n"));
        assert_eq!(shebang_for("tool.js"), Some("#!/usr/bin/env node\n"));
        assert_eq!(shebang_for("thing.rs"), None);
    }
}
