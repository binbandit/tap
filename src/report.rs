//! Result model and rendering (human text + JSON).
//!
//! `run()` is silent and returns a `Report`; rendering is a separate step so
//! the whole tool is testable and JSON output is never polluted.

use std::io::Write;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Dir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// The path did not exist and was created.
    Created,
    /// The path existed and its content/metadata changed.
    Updated,
    /// The path existed; only its times were refreshed.
    Touched,
    /// `--check`: the path exists.
    Exists,
    /// `--check`: the path does not exist.
    Missing,
    /// `--no-create`: the path does not exist and was left alone.
    Skipped,
    /// `--dry-run` previews.
    WouldCreate,
    WouldUpdate,
    WouldTouch,
    /// Something went wrong for this path.
    Error,
}

impl Action {
    pub fn verb(self) -> &'static str {
        match self {
            Action::Created => "created",
            Action::Updated => "updated",
            Action::Touched => "touched",
            Action::Exists => "exists",
            Action::Missing => "missing",
            Action::Skipped => "skipped",
            Action::WouldCreate => "would create",
            Action::WouldUpdate => "would update",
            Action::WouldTouch => "would touch",
            Action::Error => "error",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PathReport {
    pub path: String,
    pub ok: bool,
    pub action: Action,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl PathReport {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            ok: false,
            action: Action::Error,
            kind: None,
            changes: Vec::new(),
            warnings: Vec::new(),
            error: None,
            hint: None,
        }
    }

    pub fn fail(&mut self, error: impl Into<String>, hint: Option<&str>) {
        self.ok = false;
        self.action = Action::Error;
        self.error = Some(error.into());
        self.hint = hint.map(str::to_string);
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub succeeded: usize,
    pub failed: usize,
    pub results: Vec<PathReport>,
}

impl Report {
    pub fn new(results: Vec<PathReport>) -> Self {
        let succeeded = results.iter().filter(|r| r.ok).count();
        Self {
            succeeded,
            failed: results.len() - succeeded,
            results,
        }
    }

    pub fn exit_code(&self) -> i32 {
        if self.failed == 0 {
            0
        } else {
            1
        }
    }
}

/// How much to say, and how prettily.
pub struct Style {
    pub color: bool,
    /// Print success lines (TTY, --verbose, or --dry-run).
    pub chatty: bool,
    /// Print nothing but errors' exit code.
    pub quiet: bool,
}

impl Style {
    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }
    fn green(&self, t: &str) -> String {
        self.paint("32", t)
    }
    fn red(&self, t: &str) -> String {
        self.paint("31", t)
    }
    fn yellow(&self, t: &str) -> String {
        self.paint("33", t)
    }
    fn dim(&self, t: &str) -> String {
        self.paint("2", t)
    }
}

/// Render the report as JSON to `out`.
pub fn render_json(report: &Report, out: &mut impl Write) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(report).expect("report serializes");
    writeln!(out, "{json}")
}

/// Render the report for humans: successes to `out`, problems to `err`.
pub fn render_text(
    report: &Report,
    style: &Style,
    out: &mut impl Write,
    err: &mut impl Write,
) -> std::io::Result<()> {
    for r in &report.results {
        for warning in &r.warnings {
            if !style.quiet {
                writeln!(err, "{} {}", style.yellow("warning:"), warning)?;
            }
        }

        if let Some(error) = &r.error {
            writeln!(err, "{} {}: {}", style.red("✗"), r.path, error)?;
            if let Some(hint) = &r.hint {
                writeln!(err, "  {}", style.dim(&format!("hint: {hint}")))?;
            }
            continue;
        }

        if r.action == Action::Missing {
            // --check miss: meaningful even when piped.
            if !style.quiet {
                writeln!(out, "{} {:<12} {}", style.red("✗"), r.action.verb(), r.path)?;
            }
            continue;
        }

        if style.quiet || !style.chatty {
            continue;
        }

        let symbol = match r.action {
            Action::WouldCreate | Action::WouldUpdate | Action::WouldTouch => style.dim("→"),
            Action::Skipped => style.dim("•"),
            _ => style.green("✓"),
        };

        let mut line = format!("{} {:<12} {}", symbol, r.action.verb(), r.path);
        if !r.changes.is_empty() {
            line.push(' ');
            line.push_str(&style.dim(&format!("({})", r.changes.join(", "))));
        }
        writeln!(out, "{line}")?;
    }

    if report.failed > 0 && !style.quiet {
        writeln!(
            err,
            "{}",
            style.red(&format!(
                "tap: {} of {} path(s) failed",
                report.failed,
                report.results.len()
            ))
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Report {
        let mut ok = PathReport::new("a.txt");
        ok.ok = true;
        ok.action = Action::Created;
        ok.kind = Some(Kind::File);
        ok.changes.push("parents created".into());

        let mut bad = PathReport::new("b.txt");
        bad.fail("boom", Some("try harder"));

        Report::new(vec![ok, bad])
    }

    #[test]
    fn json_is_stable_and_complete() {
        let mut buf = Vec::new();
        render_json(&sample(), &mut buf).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["succeeded"], 1);
        assert_eq!(v["failed"], 1);
        assert_eq!(v["results"][0]["action"], "created");
        assert_eq!(v["results"][0]["kind"], "file");
        assert_eq!(v["results"][1]["error"], "boom");
        assert_eq!(v["results"][1]["hint"], "try harder");
    }

    #[test]
    fn text_splits_success_and_failure_streams() {
        let style = Style {
            color: false,
            chatty: true,
            quiet: false,
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        render_text(&sample(), &style, &mut out, &mut err).unwrap();
        let out = String::from_utf8(out).unwrap();
        let err = String::from_utf8(err).unwrap();

        assert!(out.contains("created"));
        assert!(out.contains("(parents created)"));
        assert!(err.contains("b.txt: boom"));
        assert!(err.contains("hint: try harder"));
        assert!(err.contains("1 of 2 path(s) failed"));
    }

    #[test]
    fn quiet_silences_stdout_but_not_errors() {
        let style = Style {
            color: false,
            chatty: true,
            quiet: true,
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        render_text(&sample(), &style, &mut out, &mut err).unwrap();
        assert!(out.is_empty());
        assert!(String::from_utf8(err).unwrap().contains("boom"));
    }
}
