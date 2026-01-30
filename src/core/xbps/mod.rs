// Author Dustin Pilgrim
// License: MIT

use crate::{config::Config, log::Log};
use std::process::ExitCode;

mod install;
mod parse;
mod plan;
mod query;

pub use plan::{plan_system_updates, SysUpdate};

pub fn search(log: &Log, cfg: Option<&Config>, installed: bool, term: &[String]) -> ExitCode {
    query::search(log, cfg, installed, term)
}

pub fn info(log: &Log, cfg: Option<&Config>, pkg: &str) -> ExitCode {
    query::info(log, cfg, pkg)
}

pub fn files(log: &Log, cfg: Option<&Config>, pkg: &str) -> ExitCode {
    query::files(log, cfg, pkg)
}

pub fn provides(log: &Log, cfg: Option<&Config>, path: &str) -> ExitCode {
    query::provides(log, cfg, path)
}

pub fn add(log: &Log, cfg: Option<&Config>, yes: bool, pkgs: &[String]) -> ExitCode {
    install::add(log, cfg, yes, pkgs)
}

pub fn rm(log: &Log, cfg: Option<&Config>, yes: bool, pkgs: &[String]) -> ExitCode {
    install::rm(log, cfg, yes, pkgs)
}

pub fn up_with_yes(log: &Log, cfg: Option<&Config>, yes: bool) -> ExitCode {
    install::up_with_yes(log, cfg, yes)
}

