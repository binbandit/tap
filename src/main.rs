use std::io::{self, IsTerminal, Write};
use std::process::{Command, ExitCode};

use clap::{CommandFactory, Parser};

use tap::cli::Cli;
use tap::report::{Kind, Report, Style};

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(shell) = cli.completions {
        let mut cmd = Cli::command();
        clap_complete::generate(shell, &mut cmd, "tap", &mut io::stdout());
        return ExitCode::SUCCESS;
    }

    let report = match tap::run(&cli) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("tap: error: {e:#}");
            return ExitCode::from(2);
        }
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut err = io::stderr().lock();

    if cli.json {
        let _ = tap::report::render_json(&report, &mut out);
    } else {
        let tty = io::stdout().is_terminal();
        let style = Style {
            color: tty && std::env::var_os("NO_COLOR").is_none(),
            chatty: tty || cli.verbose || cli.dry_run || cli.check,
            quiet: cli.quiet,
        };
        let _ = tap::report::render_text(&report, &style, &mut out, &mut err);
    }
    let _ = out.flush();

    if cli.edit && report.failed == 0 {
        open_editor(&report, &mut err);
    }

    match report.exit_code() {
        0 => ExitCode::SUCCESS,
        code => ExitCode::from(code as u8),
    }
}

/// Open the touched files in $VISUAL/$EDITOR. Best-effort: a missing or
/// failing editor is reported but never changes tap's exit status.
fn open_editor(report: &Report, err: &mut impl Write) {
    let files: Vec<&str> = report
        .results
        .iter()
        .filter(|r| r.ok && r.kind == Some(Kind::File))
        .map(|r| r.path.as_str())
        .collect();

    if files.is_empty() {
        let _ = writeln!(err, "tap: --edit: no files to open");
        return;
    }

    let editor = ["VISUAL", "EDITOR"]
        .iter()
        .filter_map(|var| std::env::var(var).ok())
        .find(|value| !value.trim().is_empty());

    let Some(editor) = editor else {
        let _ = writeln!(err, "tap: --edit: set $EDITOR (or $VISUAL) to use this");
        return;
    };

    // Allow editors with baked-in flags, e.g. EDITOR="code --wait".
    let mut parts = editor.split_whitespace();
    let program = parts.next().expect("non-empty checked above");
    let status = Command::new(program).args(parts).args(&files).status();

    if let Err(e) = status {
        let _ = writeln!(err, "tap: --edit: failed to launch '{editor}': {e}");
    }
}
