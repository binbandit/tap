//! End-to-end tests: parse real argv with clap, run the tool, inspect the
//! filesystem and the report. No process spawning needed.

use clap::Parser;
use tap::cli::Cli;
use tap::report::{Action, Report};
use tempfile::TempDir;

fn tap(dir: &TempDir, args: &[&str]) -> Report {
    let mut argv = vec!["tap".to_string()];
    for a in args {
        // Make relative paths land inside the sandbox.
        if a.starts_with('-') {
            argv.push(a.to_string());
        } else {
            argv.push(dir.path().join(a).display().to_string());
        }
    }
    let cli = Cli::try_parse_from(&argv).expect("argv parses");
    tap::run(&cli).expect("run succeeds")
}

/// Like `tap`, but for flag values that are not paths (e.g. -w TEXT).
fn tap_raw(dir: &TempDir, args: &[(&str, bool)]) -> Report {
    let mut argv = vec!["tap".to_string()];
    for (a, is_path) in args {
        if *is_path {
            argv.push(dir.path().join(a).display().to_string());
        } else {
            argv.push(a.to_string());
        }
    }
    let cli = Cli::try_parse_from(&argv).expect("argv parses");
    tap::run(&cli).expect("run succeeds")
}

#[test]
fn creates_a_file_with_parents() {
    let dir = TempDir::new().unwrap();
    let report = tap(&dir, &["deep/nested/file.txt"]);

    assert_eq!(report.failed, 0);
    assert_eq!(report.results[0].action, Action::Created);
    assert!(report.results[0]
        .changes
        .contains(&"parents created".into()));
    assert!(dir.path().join("deep/nested/file.txt").is_file());
}

#[test]
fn trailing_slash_means_directory() {
    let dir = TempDir::new().unwrap();
    let report = tap(&dir, &["build/"]);

    assert_eq!(report.failed, 0);
    assert!(dir.path().join("build").is_dir());
}

#[test]
fn dir_flag_means_directory() {
    let dir = TempDir::new().unwrap();
    tap(&dir, &["-d", "logs"]);
    assert!(dir.path().join("logs").is_dir());
}

#[test]
fn no_parents_fails_like_classic_touch() {
    let dir = TempDir::new().unwrap();
    let report = tap(&dir, &["--no-parents", "missing/file.txt"]);

    assert_eq!(report.failed, 1);
    assert!(!dir.path().join("missing/file.txt").exists());
    assert!(report.results[0].hint.is_some());
}

#[test]
fn no_create_skips_missing_without_failing() {
    let dir = TempDir::new().unwrap();
    let report = tap(&dir, &["-c", "ghost.txt"]);

    assert_eq!(report.failed, 0);
    assert_eq!(report.results[0].action, Action::Skipped);
    assert!(!dir.path().join("ghost.txt").exists());
}

#[test]
fn touching_an_existing_directory_is_fine() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("existing")).unwrap();
    let report = tap(&dir, &["existing"]);

    assert_eq!(report.failed, 0);
    assert_eq!(report.results[0].action, Action::Touched);
}

#[test]
fn dir_request_over_existing_file_fails_clearly() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("taken"), "x").unwrap();
    let report = tap(&dir, &["-d", "taken"]);

    assert_eq!(report.failed, 1);
    assert!(report.results[0]
        .error
        .as_ref()
        .unwrap()
        .contains("file already exists"));
}

#[test]
fn write_seeds_new_files_with_a_trailing_newline() {
    let dir = TempDir::new().unwrap();
    tap_raw(
        &dir,
        &[("-w", false), ("hello", false), (".gitignore", true)],
    );

    let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert_eq!(content, "hello\n");
}

#[test]
fn write_refuses_to_clobber_without_force() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("precious.txt");
    std::fs::write(&file, "do not lose me").unwrap();

    let report = tap_raw(
        &dir,
        &[("-w", false), ("new", false), ("precious.txt", true)],
    );

    assert_eq!(report.failed, 1);
    assert!(report.results[0].hint.as_ref().unwrap().contains("--force"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "do not lose me");
}

#[test]
fn force_allows_overwriting() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("precious.txt");
    std::fs::write(&file, "old").unwrap();

    let report = tap_raw(
        &dir,
        &[
            ("--force", false),
            ("-w", false),
            ("new", false),
            ("precious.txt", true),
        ],
    );

    assert_eq!(report.failed, 0);
    assert_eq!(report.results[0].action, Action::Updated);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "new\n");
}

#[test]
fn append_stitches_a_missing_newline() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("list.txt");
    std::fs::write(&file, "first").unwrap(); // note: no trailing newline

    tap_raw(
        &dir,
        &[
            ("--append", false),
            ("-w", false),
            ("second", false),
            ("list.txt", true),
        ],
    );

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "first\nsecond\n");
}

#[test]
fn append_creates_the_file_when_missing() {
    let dir = TempDir::new().unwrap();
    tap_raw(
        &dir,
        &[
            ("--append", false),
            ("-w", false),
            ("only line", false),
            ("fresh.txt", true),
        ],
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("fresh.txt")).unwrap(),
        "only line\n"
    );
}

#[test]
fn template_copies_content() {
    let dir = TempDir::new().unwrap();
    let template = dir.path().join("tmpl.txt");
    std::fs::write(&template, "boilerplate").unwrap();

    let report = tap_raw(
        &dir,
        &[
            ("--template", false),
            (template.to_str().unwrap(), false),
            ("out.txt", true),
        ],
    );

    assert_eq!(report.failed, 0);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("out.txt")).unwrap(),
        "boilerplate"
    );
}

#[test]
fn missing_template_aborts_the_whole_run() {
    let dir = TempDir::new().unwrap();
    let cli = Cli::try_parse_from([
        "tap",
        "--template",
        "/definitely/not/here.txt",
        dir.path().join("out.txt").to_str().unwrap(),
    ])
    .unwrap();

    assert!(tap::run(&cli).is_err());
    assert!(!dir.path().join("out.txt").exists());
}

#[test]
fn check_reports_and_sets_exit_code() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("real.txt"), "x").unwrap();

    let report = tap(&dir, &["--check", "real.txt", "fake.txt"]);

    assert_eq!(report.results[0].action, Action::Exists);
    assert_eq!(report.results[1].action, Action::Missing);
    assert_eq!(report.exit_code(), 1);

    let all_good = tap(&dir, &["--check", "real.txt"]);
    assert_eq!(all_good.exit_code(), 0);
}

#[test]
fn dry_run_changes_nothing_but_tells_all() {
    let dir = TempDir::new().unwrap();
    let report = tap(&dir, &["-n", "a/b/c.txt"]);

    assert_eq!(report.failed, 0);
    assert_eq!(report.results[0].action, Action::WouldCreate);
    assert!(report.results[0]
        .changes
        .contains(&"parents created".into()));
    assert!(!dir.path().join("a").exists());
}

#[test]
fn dry_run_still_surfaces_safety_errors() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("full.txt"), "content").unwrap();

    let report = tap_raw(
        &dir,
        &[
            ("-n", false),
            ("-w", false),
            ("x", false),
            ("full.txt", true),
        ],
    );
    assert_eq!(report.failed, 1);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("full.txt")).unwrap(),
        "content"
    );
}

#[test]
fn brace_expansion_creates_families() {
    let dir = TempDir::new().unwrap();
    let report = tap(&dir, &["src/{lib,main}.rs"]);

    assert_eq!(report.results.len(), 2);
    assert!(dir.path().join("src/lib.rs").is_file());
    assert!(dir.path().join("src/main.rs").is_file());
}

#[test]
fn unmatched_glob_fails_instead_of_creating_a_star_file() {
    let dir = TempDir::new().unwrap();
    let report = tap(&dir, &["*.nope"]);

    assert_eq!(report.failed, 1);
    assert!(report.results[0]
        .error
        .as_ref()
        .unwrap()
        .contains("matched nothing"));
    // Critically: no file literally named "*.nope".
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn glob_touches_all_matches() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.log"), "").unwrap();
    std::fs::write(dir.path().join("b.log"), "").unwrap();

    let report = tap(&dir, &["*.log"]);
    assert_eq!(report.results.len(), 2);
    assert_eq!(report.failed, 0);
    assert!(report.results.iter().all(|r| r.action == Action::Touched));
}

#[cfg(unix)]
#[test]
fn mode_is_applied() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    tap_raw(
        &dir,
        &[("--mode", false), ("600", false), ("secret.txt", true)],
    );

    let mode = std::fs::metadata(dir.path().join("secret.txt"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn bad_mode_aborts_before_touching_anything() {
    let dir = TempDir::new().unwrap();
    let cli = Cli::try_parse_from([
        "tap",
        "--mode",
        "999",
        dir.path().join("x.txt").to_str().unwrap(),
    ])
    .unwrap();

    assert!(tap::run(&cli).is_err());
    assert!(!dir.path().join("x.txt").exists());
}

#[cfg(unix)]
#[test]
fn exec_makes_scripts_with_shebangs() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let report = tap(&dir, &["-x", "deploy.sh"]);

    assert_eq!(report.failed, 0);
    let path = dir.path().join("deploy.sh");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "#!/usr/bin/env bash\n"
    );
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_ne!(mode & 0o111, 0, "exec bits should be set");
}

#[cfg(unix)]
#[test]
fn exec_does_not_inject_shebang_into_existing_files() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("existing.sh");
    std::fs::write(&file, "echo hi\n").unwrap();

    tap(&dir, &["-x", "existing.sh"]);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "echo hi\n");
}

#[test]
fn explicit_timestamp_is_applied_to_mtime() {
    let dir = TempDir::new().unwrap();
    tap_raw(
        &dir,
        &[("-t", false), ("@1700000000", false), ("dated.txt", true)],
    );

    let mtime = filetime::FileTime::from_last_modification_time(
        &std::fs::metadata(dir.path().join("dated.txt")).unwrap(),
    );
    assert_eq!(mtime.unix_seconds(), 1_700_000_000);
}

#[test]
fn reference_copies_times() {
    let dir = TempDir::new().unwrap();
    let source = dir.path().join("source.txt");
    std::fs::write(&source, "x").unwrap();
    filetime::set_file_times(
        &source,
        filetime::FileTime::from_unix_time(1_600_000_000, 0),
        filetime::FileTime::from_unix_time(1_600_000_000, 0),
    )
    .unwrap();

    tap_raw(
        &dir,
        &[
            ("-r", false),
            (source.to_str().unwrap(), false),
            ("copy.txt", true),
        ],
    );

    let md = std::fs::metadata(dir.path().join("copy.txt")).unwrap();
    assert_eq!(
        filetime::FileTime::from_last_modification_time(&md).unix_seconds(),
        1_600_000_000
    );
}

#[test]
fn mtime_only_leaves_atime_alone() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("selective.txt");
    std::fs::write(&file, "x").unwrap();
    let old = filetime::FileTime::from_unix_time(1_500_000_000, 0);
    filetime::set_file_times(&file, old, old).unwrap();

    tap_raw(
        &dir,
        &[
            ("-m", false),
            ("-t", false),
            ("@1700000000", false),
            ("selective.txt", true),
        ],
    );

    let md = std::fs::metadata(&file).unwrap();
    assert_eq!(
        filetime::FileTime::from_last_modification_time(&md).unix_seconds(),
        1_700_000_000
    );
    assert_eq!(
        filetime::FileTime::from_last_access_time(&md).unix_seconds(),
        1_500_000_000
    );
}

#[test]
fn touching_existing_file_refreshes_times() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("stale.txt");
    std::fs::write(&file, "x").unwrap();
    let old = filetime::FileTime::from_unix_time(1_000_000_000, 0);
    filetime::set_file_times(&file, old, old).unwrap();

    let report = tap(&dir, &["stale.txt"]);
    assert_eq!(report.results[0].action, Action::Touched);

    let md = std::fs::metadata(&file).unwrap();
    let now = filetime::FileTime::now().unix_seconds();
    let mtime = filetime::FileTime::from_last_modification_time(&md).unix_seconds();
    assert!((now - mtime).abs() < 10, "mtime should be refreshed to now");
}

#[test]
fn mixed_success_and_failure_exits_nonzero_but_finishes_the_batch() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("blocker"), "x").unwrap();

    let report = tap(&dir, &["ok.txt", "blocker/"]);
    assert_eq!(report.succeeded, 1);
    assert_eq!(report.failed, 1);
    assert_eq!(report.exit_code(), 1);
    assert!(dir.path().join("ok.txt").exists());
}
