// Author Dustin Pilgrim
// License: MIT

use crate::{
    cli::{Cli, Cmd, PkgCmd, SrcBuildFlags, SrcCmd},
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

        Cmd::Search { term } => xbps::search(log, cfg.as_ref(), false, &term),

        Cmd::Info { pkg } => xbps::info(log, cfg.as_ref(), &pkg),

        Cmd::Files { pkg } => xbps::files(log, cfg.as_ref(), &pkg),

        Cmd::List { term } => xbps::list(log, cfg.as_ref(), term.as_deref()),

        Cmd::Owns { path } => xbps::owns(log, cfg.as_ref(), &path),

        Cmd::Add {
            yes,
            automatic,
            config_dir,
            cachedir,
            debug,
            download_only,
            force,
            ignore_conf_repos,
            ignore_file_conflicts,
            unpack_only,
            memory_sync,
            dry_run,
            repositories,
            rootdir,
            reproducible,
            staging,
            no_sync,
            update,
            xbps_verbose,
            pkgs,
            xbps_args,
        } => xbps::add(
            log,
            cfg.as_ref(),
            xbps::AddOptions {
                yes,
                automatic,
                config_dir,
                cachedir,
                debug,
                download_only,
                force,
                ignore_conf_repos,
                ignore_file_conflicts,
                unpack_only,
                memory_sync,
                dry_run,
                repositories,
                rootdir,
                reproducible,
                staging,
                sync: !no_sync,
                update,
                xbps_verbose,
                xbps_args,
            },
            &pkgs,
        ),

        Cmd::Rm {
            yes,
            config_dir,
            cachedir,
            debug,
            force_revdeps,
            force,
            dry_run,
            clean_cache,
            orphans,
            no_recursive,
            rootdir,
            xbps_verbose,
            xbps_args,
            pkgs,
        } => xbps::rm(
            log,
            cfg.as_ref(),
            xbps::RmOptions {
                yes,
                config_dir,
                cachedir,
                debug,
                force_revdeps,
                force,
                dry_run,
                clean_cache,
                orphans,
                recursive: !no_recursive,
                rootdir,
                xbps_verbose,
                xbps_args,
            },
            &pkgs,
        ),

        Cmd::Up {
            all,
            dry_run,
            force,
            yes,
            local,
        } => {
            // remote = true unless --local was passed
            let remote = !local;

            // vx up — system only
            if !all {
                let sys_plan = match xbps::plan_system_updates_fresh(log, cfg.as_ref()) {
                    Ok(v) => v,
                    Err(e) => {
                        log.error(e);
                        return ExitCode::from(1);
                    }
                };

                if sys_plan.is_empty() {
                    log.info("system already up to date.");
                    return ExitCode::SUCCESS;
                }

                if dry_run {
                    println!("system update plan:");
                    for u in sys_plan {
                        println!("  {}  {} → {}", u.name, u.from, u.to);
                    }
                    return ExitCode::SUCCESS;
                }

                return xbps::up_with_yes(log, cfg.as_ref(), yes);
            }

            // vx up -a — system + source
            let sys_plan = match xbps::plan_system_updates_fresh(log, cfg.as_ref()) {
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
                remote,
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
                    println!("vx: everything up to date.");
                }
                return ExitCode::SUCCESS;
            }

            if dry_run {
                return ExitCode::SUCCESS;
            }

            if !yes && !source::confirm_once("Proceed?") {
                log.info("aborted.");
                return ExitCode::SUCCESS;
            }

            // System first, then source.
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
                    dry_run: false,
                    force: true,
                    yes: true,
                    local: !remote,
                    build: SrcBuildFlags::default(),
                    pkgs: pkgs_to_update,
                    xbps_src_args: Vec::new(),
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
