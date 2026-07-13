use clap::{ArgGroup, Parser};

const AFTER_HELP: &str = "\
EXAMPLES:
  tap notes.md                        create a file (or update its times)
  tap src/deep/nested/file.rs         parent directories are created for you
  tap build/ logs/                    trailing slash creates a directory
  tap src/{lib,main,tests}.rs         brace expansion, on every shell
  tap shot_{01..12}.png               numeric ranges too
  tap config.json{,.bak}              quick backup-name twins

  tap -w 'node_modules' .gitignore    seed a new file with content
  tap --append -w 'dist' .gitignore   append a line (newline handled for you)
  tap --template LICENSE-MIT LICENSE  start from a template file

  tap -x deploy.sh                    executable script, shebang included
  tap -e TODO.md                      create it, then open it in $EDITOR

  tap -t yesterday photo.jpg          friendly timestamps
  tap -t '2024-01-01 09:30' a.log     ISO dates, local timezone
  tap -t -2h report.txt               relative offsets: -2h, +30m, '3 days ago'
  tap -r original.txt copy.txt        copy times from a reference file

  tap --check 'config/*.yml'          do the files exist? (exit 1 if not)
  tap -n src/{a,b}/mod.rs             dry run: see what would happen
  tap --json *.txt                    machine-readable results

The full manual lives at: https://github.com/crazywolf132/tap";

/// tap - make paths exist.
///
/// A modern, friendly replacement for touch: create files and directories
/// (parents included), seed content safely, set permissions and timestamps,
/// and get structured output when you script it.
#[derive(Parser, Debug)]
#[command(
    name = "tap",
    version,
    about = "Make paths exist - a modern, friendly replacement for touch",
    after_help = AFTER_HELP,
    group(ArgGroup::new("content").args(["write", "template"]).multiple(false)),
    group(ArgGroup::new("when").args(["at", "reference"]).multiple(false))
)]
pub struct Cli {
    /// Paths to create or update. A trailing '/' means directory.
    /// Braces expand ({a,b}, {1..5}) and globs match existing files.
    #[arg(required_unless_present = "completions", value_name = "PATH")]
    pub paths: Vec<String>,

    /// Treat all paths as directories
    #[arg(short, long, alias = "mkdir", help_heading = "What to make")]
    pub dir: bool,

    /// Don't create anything; only update paths that already exist
    #[arg(short = 'c', long, help_heading = "What to make")]
    pub no_create: bool,

    /// Fail if a parent directory is missing (classic touch behaviour)
    #[arg(long, alias = "no-parent", help_heading = "What to make")]
    pub no_parents: bool,

    /// Write TEXT into the file (a trailing newline is added if missing)
    #[arg(short, long, value_name = "TEXT", help_heading = "Content")]
    pub write: Option<String>,

    /// Copy the contents of FILE into the target
    #[arg(long, value_name = "FILE", help_heading = "Content")]
    pub template: Option<String>,

    /// Append instead of writing; separators are handled for you
    #[arg(long, requires = "content", help_heading = "Content")]
    pub append: bool,

    /// Allow --write/--template to replace a file that already has content
    #[arg(long, help_heading = "Content")]
    pub force: bool,

    /// Set times: '2024-01-01 09:30', 'yesterday', '-2h', '@1700000000', ...
    #[arg(
        short = 't',
        long,
        alias = "timestamp",
        value_name = "WHEN",
        allow_hyphen_values = true,
        help_heading = "Times"
    )]
    pub at: Option<String>,

    /// Copy access/modification times from FILE
    #[arg(
        short = 'r',
        long,
        alias = "ref",
        value_name = "FILE",
        help_heading = "Times"
    )]
    pub reference: Option<String>,

    /// Change only the access time
    #[arg(short = 'a', long = "atime", help_heading = "Times")]
    pub atime: bool,

    /// Change only the modification time
    #[arg(short = 'm', long = "mtime", help_heading = "Times")]
    pub mtime: bool,

    /// Set permissions (octal, e.g. 644 or 2755)
    #[arg(
        long,
        alias = "chmod",
        value_name = "OCTAL",
        help_heading = "Permissions"
    )]
    pub mode: Option<String>,

    /// Apply --mode recursively to directory contents
    #[arg(short = 'R', long, requires = "mode", help_heading = "Permissions")]
    pub recursive: bool,

    /// Make the file executable; new scripts get a fitting shebang
    #[arg(short = 'x', long, help_heading = "Permissions")]
    pub exec: bool,

    /// Report whether each path exists, change nothing (exit 1 if any are missing)
    #[arg(
        long,
        alias = "exists",
        help_heading = "Behaviour",
        conflicts_with_all = [
            "write", "template", "append", "force", "at", "reference",
            "atime", "mtime", "mode", "recursive", "exec", "edit", "dry_run"
        ]
    )]
    pub check: bool,

    /// Show what would happen without touching the filesystem
    #[arg(short = 'n', long, help_heading = "Behaviour")]
    pub dry_run: bool,

    /// Open the touched files in $VISUAL/$EDITOR afterwards
    #[arg(short = 'e', long, help_heading = "Behaviour", conflicts_with_all = ["json", "dry_run"])]
    pub edit: bool,

    /// Print nothing on success
    #[arg(short, long, help_heading = "Output", conflicts_with = "verbose")]
    pub quiet: bool,

    /// Explain everything that happens, even when piped
    #[arg(short, long, help_heading = "Output")]
    pub verbose: bool,

    /// Emit machine-readable JSON results on stdout
    #[arg(long, help_heading = "Output", conflicts_with_all = ["quiet", "verbose"])]
    pub json: bool,

    /// Print shell completions and exit
    #[arg(long, value_name = "SHELL", value_enum, help_heading = "Output")]
    pub completions: Option<clap_complete::Shell>,
}

impl Cli {
    /// A baseline Cli for tests and library callers: `tap <paths>` with defaults.
    pub fn for_paths(paths: Vec<String>) -> Self {
        Self {
            paths,
            dir: false,
            no_create: false,
            no_parents: false,
            write: None,
            template: None,
            append: false,
            force: false,
            at: None,
            reference: None,
            atime: false,
            mtime: false,
            mode: None,
            recursive: false,
            exec: false,
            check: false,
            dry_run: false,
            edit: false,
            quiet: false,
            verbose: false,
            json: false,
            completions: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn append_requires_a_content_source() {
        assert!(Cli::try_parse_from(["tap", "--append", "a.txt"]).is_err());
        assert!(Cli::try_parse_from(["tap", "--append", "-w", "hi", "a.txt"]).is_ok());
    }

    #[test]
    fn recursive_requires_mode() {
        assert!(Cli::try_parse_from(["tap", "-R", "dir/"]).is_err());
        assert!(Cli::try_parse_from(["tap", "-R", "--mode", "755", "dir/"]).is_ok());
    }

    #[test]
    fn check_conflicts_with_mutating_flags() {
        assert!(Cli::try_parse_from(["tap", "--check", "-w", "hi", "a.txt"]).is_err());
        assert!(Cli::try_parse_from(["tap", "--check", "a.txt"]).is_ok());
    }

    #[test]
    fn at_and_reference_are_exclusive() {
        assert!(Cli::try_parse_from(["tap", "-t", "now", "-r", "b", "a"]).is_err());
    }

    #[test]
    fn at_accepts_leading_hyphen_offsets() {
        let cli = Cli::try_parse_from(["tap", "-t", "-2h", "a.txt"]).unwrap();
        assert_eq!(cli.at.as_deref(), Some("-2h"));
    }

    #[test]
    fn touch_style_shorts_are_honoured() {
        let cli = Cli::try_parse_from(["tap", "-a", "-m", "-c", "a.txt"]).unwrap();
        assert!(cli.atime && cli.mtime && cli.no_create);
    }
}
