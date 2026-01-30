// Author Dustin Pilgrim
// License: MIT

use crate::{cli::SrcCmd, config::Config, log::Log, managed};
use std::{path::PathBuf, process::ExitCode};

use crate::core::xbps::SysUpdate;

mod add;
mod git;
mod plan;
mod resolve;
mod search;
mod util;
mod xbps_src;

pub use plan::{plan_src_updates, SrcUpdate};

pub fn dispatch_src(
    log: &Log,
    voidpkgs_override: Option<PathBuf>,
    cfg: Option<&Config>,
    cmd: SrcCmd,
) -> ExitCode {
    if let SrcCmd::Add { force, rebuild, .. } = &cmd {
        if *force && *rebuild {
            log.error("use either --force or --rebuild, not both");
            return ExitCode::from(2);
        }
    }

    let resolved = match resolve::resolve_voidpkgs(voidpkgs_override, cfg) {
        Ok(r) => r,
        Err(e) => {
            log.error(e);
            return ExitCode::from(2);
        }
    };

    let should_sync = matches!(&cmd, SrcCmd::Up { .. })
        || matches!(&cmd, SrcCmd::Add { rebuild: true, .. });

    if should_sync {
        if let Err(e) = git::sync_voidpkgs(log, &resolved.voidpkgs) {
            log.error(e);
            return ExitCode::from(1);
        }
    }

    match cmd {
        SrcCmd::Search { installed, term } => search::src_search(log, &resolved, installed, &term),

        SrcCmd::Build { pkgs } => xbps_src::build(log, &resolved, &pkgs),
        SrcCmd::Clean { pkgs } => xbps_src::clean(log, &resolved, &pkgs),
        SrcCmd::Lint { pkgs } => xbps_src::lint(log, &resolved, &pkgs),

        SrcCmd::Add {
            force,
            rebuild,
            yes,
            pkgs,
        } => {
            let code = if rebuild {
                // local rebuild (current behavior)
                xbps_src::src_up(log, &resolved, yes, false, &pkgs)
            } else {
                add::add_from_local_repo(log, &resolved, force, yes, &pkgs)
            };

            if code == ExitCode::SUCCESS {
                if let Err(e) = managed::add_managed(&pkgs.to_vec()) {
                    log.warn(format!("failed to update managed-src list: {e}"));
                }
            }
            code
        }

        SrcCmd::Up {
            all,
            dry_run,
            force,
            yes,
            remote,
            pkgs,
        } => {
            let target = if all {
                match managed::load_managed() {
                    Ok(v) => v,
                    Err(e) => {
                        log.error(e);
                        return ExitCode::from(2);
                    }
                }
            } else {
                pkgs
            };

            if target.is_empty() {
                log.info("no source packages specified.");
                if all {
                    log.info("hint: install one with `vx src add <pkg>`");
                } else {
                    log.info("hint: use `vx src up <pkg...>` or `vx src up --all`");
                }
                return ExitCode::SUCCESS;
            }

            let plan = match plan::plan_src_updates_with_resolved(log, &resolved, &target, force) {
                Ok(v) => v,
                Err(e) => {
                    log.error(e);
                    return ExitCode::from(1);
                }
            };

            if plan.is_empty() {
                log.info("all source packages are already up to date.");
                return ExitCode::SUCCESS;
            }

            util::print_src_plan_summary(log, &plan);

            if dry_run {
                return ExitCode::SUCCESS;
            }

            if !yes {
                if !util::confirm_once("Proceed?") {
                    log.info("aborted.");
                    return ExitCode::SUCCESS;
                }
            }

            let pkgs_to_update: Vec<String> = plan.iter().map(|p| p.name.clone()).collect();
            xbps_src::src_up(log, &resolved, yes, remote, &pkgs_to_update)
        }
    }
}

// Re-export these for core/mod.rs convenience
pub fn print_up_all_summary(log: &Log, sys: &[SysUpdate], src: &[SrcUpdate]) {
    util::print_up_all_summary(log, sys, src)
}

pub fn confirm_once(prompt: &str) -> bool {
    util::confirm_once(prompt)
}

