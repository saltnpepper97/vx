// Author Dustin Pilgrim
// License: MIT

use crate::{config::Config, log::Log};
use std::process::ExitCode;

mod install;
mod parse;
mod plan;
mod query;

pub use plan::{plan_system_updates_fresh, SysUpdate};

pub fn search(log: &Log, cfg: Option<&Config>, installed: bool, term: &[String]) -> ExitCode {
    query::search(log, cfg, installed, term)
}

pub fn info(log: &Log, cfg: Option<&Config>, pkg: &str) -> ExitCode {
    query::info(log, cfg, pkg)
}

pub fn files(log: &Log, cfg: Option<&Config>, pkg: &str) -> ExitCode {
    query::files(log, cfg, pkg)
}

/// `vx owns <path>` — who owns this file (xbps-query -o)
pub fn owns(log: &Log, cfg: Option<&Config>, path: &str) -> ExitCode {
    query::owns(log, cfg, path)
}

/// `vx list [term]` — list installed packages (optionally filtered)
pub fn list(log: &Log, cfg: Option<&Config>, term: Option<&str>) -> ExitCode {
    query::list(log, cfg, term)
}

pub fn add(log: &Log, cfg: Option<&Config>, yes: bool, pkgs: &[String]) -> ExitCode {
    install::add(log, cfg, yes, pkgs)
}

/// `vx rm <pkgs...> [--orphans]`
///
/// - Always removes the listed pkgs.
/// - If `orphans` is true, runs an additional orphan cleanup pass.
pub fn rm(
    log: &Log,
    cfg: Option<&Config>,
    yes: bool,
    orphans: bool,
    pkgs: &[String],
) -> ExitCode {
    install::rm(log, cfg, yes, orphans, pkgs)
}

pub fn up_with_yes(log: &Log, cfg: Option<&Config>, yes: bool) -> ExitCode {
    install::up_with_yes(log, cfg, yes)
}
