//! tap - make paths exist.
//!
//! The library exposes the whole tool as `run(&Cli) -> Result<Report>` so it
//! can be tested end-to-end without spawning a process, and so `run` itself
//! never prints: rendering lives in [`report`].

pub mod cli;
pub mod expand;
pub mod mode;
pub mod ops;
pub mod report;
pub mod times;

use anyhow::Result;

use cli::Cli;
use ops::Plan;
use report::Report;

/// Execute an invocation. Usage-level problems (bad --mode, unreadable
/// --template, unparseable --at) abort the whole run with `Err`; per-path
/// outcomes, good and bad, are collected in the returned [`Report`].
pub fn run(cli: &Cli) -> Result<Report> {
    let plan = Plan::prepare(cli)?;
    let targets = expand::expand_all(&cli.paths);

    let results = targets
        .iter()
        .map(|target| ops::process(cli, &plan, target))
        .collect();

    Ok(Report::new(results))
}
