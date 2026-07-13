//! Argument expansion: brace patterns and globs.
//!
//! Braces expand everywhere (`src/{a,b}.rs`, `img_{01..04}.png`) so tap
//! behaves the same on bash, PowerShell, and cmd. Glob patterns match
//! existing files; a pattern that matches nothing is surfaced as such
//! instead of silently becoming a file named `*.txt`.

use std::collections::HashSet;
use std::path::PathBuf;

/// One concrete path tap will operate on, plus how we got it.
#[derive(Debug, Clone)]
pub struct Target {
    /// The argument as the user spelled it (post brace-expansion).
    pub given: String,
    pub path: PathBuf,
    /// True when `given` contained glob metacharacters but matched nothing.
    pub unmatched_pattern: bool,
    pub warnings: Vec<String>,
}

impl Target {
    fn literal(given: String) -> Self {
        Self {
            path: PathBuf::from(&given),
            given,
            unmatched_pattern: false,
            warnings: Vec::new(),
        }
    }
}

/// Expand every CLI path argument into concrete targets, deduplicated
/// while preserving order.
pub fn expand_all(inputs: &[String]) -> Vec<Target> {
    let mut out: Vec<Target> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for raw in inputs {
        for braced in expand_braces(raw) {
            for target in expand_glob(braced) {
                if seen.insert(target.path.clone()) {
                    out.push(target);
                }
            }
        }
    }

    out
}

fn expand_glob(given: String) -> Vec<Target> {
    if !given.contains(['*', '?', '[']) {
        return vec![Target::literal(given)];
    }

    match glob::glob(&given) {
        Ok(entries) => {
            let mut warnings = Vec::new();
            let mut matched = Vec::new();
            for entry in entries {
                match entry {
                    Ok(path) => matched.push(path),
                    Err(e) => warnings.push(format!(
                        "could not read '{}': {}",
                        e.path().display(),
                        e.error()
                    )),
                }
            }

            if matched.is_empty() {
                let mut t = Target::literal(given);
                t.unmatched_pattern = true;
                t.warnings = warnings;
                return vec![t];
            }

            matched
                .into_iter()
                .enumerate()
                .map(|(i, path)| Target {
                    given: given.clone(),
                    path,
                    unmatched_pattern: false,
                    warnings: if i == 0 { warnings.clone() } else { Vec::new() },
                })
                .collect()
        }
        // Not a valid pattern ("[abc" is a perfectly good filename): treat literally.
        Err(_) => vec![Target::literal(given)],
    }
}

/// Largest range we will expand ({1..100000} is almost certainly a typo).
const MAX_RANGE: i64 = 10_000;

/// Bash-style brace expansion: alternatives `{a,b}`, numeric ranges
/// `{1..5}` (with zero-padding), and single-char ranges `{a..e}`.
/// Nested and sequential groups compose. Anything non-expandable is
/// left verbatim, so `weird{name` is still a valid filename.
pub fn expand_braces(input: &str) -> Vec<String> {
    let mut search_from = 0;
    while let Some((pre, body, post)) = split_brace(input, search_from) {
        if let Some(alternatives) = brace_alternatives(body) {
            let mut out = Vec::new();
            for alt in alternatives {
                let combined = format!("{pre}{alt}{post}");
                out.extend(expand_braces(&combined));
            }
            return out;
        }
        // This brace group wasn't expandable ({}, {single}); look further right.
        search_from = input.len() - post.len();
    }
    vec![input.to_string()]
}

/// Find the first balanced `{...}` group starting at or after `from`.
/// Returns (prefix, body-without-braces, suffix).
fn split_brace(s: &str, from: usize) -> Option<(&str, &str, &str)> {
    let bytes = s.as_bytes();
    let open = bytes[from.min(bytes.len())..]
        .iter()
        .position(|&b| b == b'{')
        .map(|i| i + from)?;

    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[..open], &s[open + 1..i], &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

fn brace_alternatives(body: &str) -> Option<Vec<String>> {
    // Split on top-level commas (commas inside nested braces don't count).
    let mut parts: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for c in body.chars() {
        match c {
            '{' => {
                depth += 1;
                current.push(c);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);

    if parts.len() > 1 {
        return Some(parts);
    }

    // No commas: maybe a range like {1..5}, {01..10}, or {a..e}.
    range_alternatives(body)
}

fn range_alternatives(body: &str) -> Option<Vec<String>> {
    let (start, end) = body.split_once("..")?;

    // Single-character ranges: {a..e}, {Z..A}.
    if start.len() == 1 && end.len() == 1 {
        let (a, b) = (start.chars().next()?, end.chars().next()?);
        if a.is_ascii_alphabetic() && b.is_ascii_alphabetic() {
            let (a, b) = (a as u8, b as u8);
            let range: Vec<String> = if a <= b {
                (a..=b).map(|c| (c as char).to_string()).collect()
            } else {
                (b..=a).rev().map(|c| (c as char).to_string()).collect()
            };
            return Some(range);
        }
    }

    // Numeric ranges, with zero-padding when the user wrote {01..10}.
    let a: i64 = start.parse().ok()?;
    let b: i64 = end.parse().ok()?;
    if (a - b).abs() >= MAX_RANGE {
        return None;
    }

    let padded = |s: &str| s.len() > 1 && (s.starts_with('0') || s.starts_with("-0"));
    let width = if padded(start) || padded(end) {
        start.len().max(end.len())
    } else {
        0
    };

    let fmt = |n: i64| {
        if width > 0 {
            format!("{n:0width$}")
        } else {
            n.to_string()
        }
    };

    let range: Vec<String> = if a <= b {
        (a..=b).map(fmt).collect()
    } else {
        (b..=a).rev().map(fmt).collect()
    };
    Some(range)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn expand(s: &str) -> Vec<String> {
        expand_braces(s)
    }

    #[test]
    fn plain_strings_pass_through() {
        assert_eq!(expand("plain.txt"), vec!["plain.txt"]);
    }

    #[test]
    fn simple_alternatives() {
        assert_eq!(expand("src/{a,b}.rs"), vec!["src/a.rs", "src/b.rs"]);
    }

    #[test]
    fn empty_alternative_makes_backup_twins() {
        assert_eq!(expand("cfg.json{,.bak}"), vec!["cfg.json", "cfg.json.bak"]);
    }

    #[test]
    fn nested_and_sequential_groups() {
        assert_eq!(expand("{a,b{1,2}}"), vec!["a", "b1", "b2"]);
        assert_eq!(expand("{a,b}{1,2}"), vec!["a1", "a2", "b1", "b2"]);
    }

    #[test]
    fn numeric_ranges_with_padding() {
        assert_eq!(expand("f{1..3}"), vec!["f1", "f2", "f3"]);
        assert_eq!(expand("f{01..03}"), vec!["f01", "f02", "f03"]);
        assert_eq!(expand("f{3..1}"), vec!["f3", "f2", "f1"]);
    }

    #[test]
    fn char_ranges() {
        assert_eq!(expand("{a..d}"), vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn non_expandable_braces_stay_literal() {
        assert_eq!(expand("{single}"), vec!["{single}"]);
        assert_eq!(expand("un{closed"), vec!["un{closed"]);
        assert_eq!(expand("{}"), vec!["{}"]);
    }

    #[test]
    fn later_groups_expand_even_after_a_literal_one() {
        assert_eq!(expand("{x}{1,2}"), vec!["{x}1", "{x}2"]);
    }

    #[test]
    fn huge_ranges_are_refused() {
        assert_eq!(expand("{1..99999}"), vec!["{1..99999}"]);
    }

    #[test]
    fn globs_match_existing_files() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("one.txt"), "x").unwrap();
        std::fs::write(dir.path().join("two.txt"), "x").unwrap();

        let pattern = format!("{}/*.txt", dir.path().display());
        let targets = expand_all(&[pattern]);
        assert_eq!(targets.len(), 2);
        assert!(targets.iter().all(|t| !t.unmatched_pattern));
    }

    #[test]
    fn unmatched_glob_is_flagged_not_silently_literal() {
        let dir = tempdir().unwrap();
        let pattern = format!("{}/*.nope", dir.path().display());
        let targets = expand_all(&[pattern]);
        assert_eq!(targets.len(), 1);
        assert!(targets[0].unmatched_pattern);
    }

    #[test]
    fn invalid_glob_is_treated_as_a_literal_filename() {
        let targets = expand_all(&["[abc".to_string()]);
        assert_eq!(targets.len(), 1);
        assert!(!targets[0].unmatched_pattern);
        assert_eq!(targets[0].path, PathBuf::from("[abc"));
    }

    #[test]
    fn duplicates_are_removed_in_order() {
        let targets = expand_all(&["a".to_string(), "{a,b}".to_string()]);
        let names: Vec<_> = targets.iter().map(|t| t.given.clone()).collect();
        assert_eq!(names, vec!["a", "b"]);
    }
}
