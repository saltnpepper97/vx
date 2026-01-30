// Author Dustin Pilgrim
// License: MIT

use crate::{
    cli::{Cli, Cmd, PkgCmd, SrcCmd},
    config::Config,
    log::Log,
};
use std::process::ExitCode;

pub mod pkg;
pub mod source;
pub mod status;
pub mod xbps;

pub fn dispatch(log: &Log, cli: Cli, cfg: Option<Config>) -> ExitCode {
    let voidpkgs_override = cli.voidpkgs.clone();

    match cli.cmd {
        Cmd::Status => status::run_status(log, &cli, cfg.as_ref()),

        Cmd::Search { installed, term } => xbps::search(log, cfg.as_ref(), installed, &term),
        Cmd::Info { pkg } => xbps::info(log, cfg.as_ref(), &pkg),
        Cmd::Files { pkg } => xbps::files(log, cfg.as_ref(), &pkg),
        Cmd::Provides { path } => xbps::provides(log, cfg.as_ref(), &path),

        Cmd::Add { yes, pkgs } => xbps::add(log, cfg.as_ref(), yes, &pkgs),
        Cmd::Rm { yes, pkgs } => xbps::rm(log, cfg.as_ref(), yes, &pkgs),

        Cmd::Up {
            all,
            dry_run,
            force,
            yes,
        } => {
            // vx up (system only)
            if !all {
                if dry_run {
                    let sys_plan = match xbps::plan_system_updates(log, cfg.as_ref()) {
                        Ok(v) => v,
                        Err(e) => {
                            log.error(e);
                            return ExitCode::from(1);
                        }
                    };

                    if sys_plan.is_empty() {
                        log.info("already up to date.");
                        return ExitCode::SUCCESS;
                    }

                    log.info("system update plan");
                    for u in sys_plan {
                        println!("  {}  {} → {}", u.name, u.from, u.to);
                    }
                    return ExitCode::SUCCESS;
                }

                return xbps::up_with_yes(log, cfg.as_ref(), yes);
            }

            // vx up -a (system + source)
            let sys_plan = match xbps::plan_system_updates(log, cfg.as_ref()) {
                Ok(v) => v,
                Err(e) => {
                    log.error(e);
                    return ExitCode::from(1);
                }
            };

            let src_plan = match source::plan_src_updates(
                log,
                voidpkgs_override.clone(),
                cfg.as_ref(),
                None,
                force,
            ) {
                Ok(v) => v,
                Err(e) => {
                    log.error(e);
                    return ExitCode::from(1);
                }
            };

            source::print_up_all_summary(log, &sys_plan, &src_plan);

            if sys_plan.is_empty() && src_plan.is_empty() {
                if !log.quiet {
                    println!("vx: already up to date.");
                }
                return ExitCode::SUCCESS;
            }

            if dry_run {
                return ExitCode::SUCCESS;
            }

            if !yes {
                if !source::confirm_once("Proceed?") {
                    log.info("aborted.");
                    return ExitCode::SUCCESS;
                }
            }

            // Apply system updates first, then source.
            if !sys_plan.is_empty() {
                let c = xbps::up_with_yes(log, cfg.as_ref(), true);
                if c != ExitCode::SUCCESS {
                    return c;
                }
            }

            let pkgs_to_update: Vec<String> = src_plan.iter().map(|p| p.name.clone()).collect();
            if pkgs_to_update.is_empty() {
                return ExitCode::SUCCESS;
            }

            source::dispatch_src(
                log,
                voidpkgs_override,
                cfg.as_ref(),
                SrcCmd::Up {
                    all: false,
                    dry_run: false,
                    force: true,
                    yes: true,
                    pkgs: pkgs_to_update,
                },
            )
        }

        Cmd::Src { cmd } => source::dispatch_src(log, voidpkgs_override, cfg.as_ref(), cmd),

        Cmd::Pkg {
            name,
            gensum,
            force,
            content,
            arch,
            hostdir,
            cmd,
        } => {
            if let Some(sub) = cmd {
                match sub {
                    PkgCmd::New { name } => {
                        pkg::pkg_new(log, voidpkgs_override, cfg.as_ref(), &name)
                    }
                }
            } else if gensum {
                let Some(pkg) = name else {
                    log.error("usage: vx pkg <name> --gensum");
                    return ExitCode::from(2);
                };
                pkg::pkg_gensum(
                    log,
                    voidpkgs_override,
                    cfg.as_ref(),
                    &pkg,
                    force,
                    content,
                    arch.as_deref(),
                    hostdir.as_ref(),
                )
            } else {
                log.error("usage: vx pkg <name> --gensum   OR   vx pkg new <name>");
                ExitCode::from(2)
            }
        }
    }
}

