// Author Dustin Pilgrim
// License: MIT

use crate::{config::Config, log::Log};
use std::path::PathBuf;
use std::process::ExitCode;

mod install;
mod parse;
mod plan;
mod query;

pub use plan::{plan_system_updates_fresh, SysUpdate};

#[derive(Debug, Clone)]
pub struct AddOptions {
    pub yes: bool,
    pub automatic: bool,
    pub config_dir: Option<PathBuf>,
    pub cachedir: Option<PathBuf>,
    pub debug: bool,
    pub download_only: bool,
    pub force: u8,
    pub ignore_conf_repos: bool,
    pub ignore_file_conflicts: bool,
    pub unpack_only: bool,
    pub memory_sync: bool,
    pub dry_run: bool,
    pub repositories: Vec<String>,
    pub rootdir: Option<PathBuf>,
    pub reproducible: bool,
    pub staging: bool,
    pub sync: bool,
    pub update: bool,
    pub xbps_verbose: bool,
    pub xbps_args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RmOptions {
    pub yes: bool,
    pub config_dir: Option<PathBuf>,
    pub cachedir: Option<PathBuf>,
    pub debug: bool,
    pub force_revdeps: bool,
    pub force: bool,
    pub dry_run: bool,
    pub clean_cache: u8,
    pub orphans: bool,
    pub recursive: bool,
    pub rootdir: Option<PathBuf>,
    pub xbps_verbose: bool,
    pub xbps_args: Vec<String>,
}

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

pub fn add(log: &Log, cfg: Option<&Config>, opts: AddOptions, pkgs: &[String]) -> ExitCode {
    install::add(log, cfg, opts, pkgs)
}

/// `vx rm <pkgs...> [--orphans]`
///
/// - Always removes the listed pkgs.
/// - If `orphans` is true, runs an additional orphan cleanup pass.
pub fn rm(log: &Log, cfg: Option<&Config>, opts: RmOptions, pkgs: &[String]) -> ExitCode {
    install::rm(log, cfg, opts, pkgs)
}

pub fn up_with_yes(log: &Log, cfg: Option<&Config>, yes: bool) -> ExitCode {
    install::up_with_yes(log, cfg, yes)
}
