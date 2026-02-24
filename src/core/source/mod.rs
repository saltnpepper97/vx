// Author Dustin Pilgrim
// License: MIT

use crate::{
    cli::SrcCmd,
    config::Config,
    log::Log,
    managed,
};
use std::{
    io::{self, Write},
    path::PathBuf,
    process::{Command, ExitCode, Stdio},
};

pub mod add;
pub mod git;
pub mod plan;
pub mod resolve;
pub mod xbps_src;

pub use plan::{plan_src_updates, SrcUpdate};

/// Print a combined system + source update summary for `vx up -a`.
pub fn print_up_all_summary(
    log: &Log,
    sys: &[crate::core::xbps::SysUpdate],
    src: &[SrcUpdate],
) {
    if sys.is_empty() && src.is_empty() {
        return;
    }

    if !log.quiet {
        println!("update plan:");
    }

    if !sys.is_empty() {
        if !log.quiet {
            println!("  system ({}):", sys.len());
        }
        for u in sys {
            println!("    {}  {} → {}", u.name, u.from, u.to);
        }
    }

    if !src.is_empty() {
        if !log.quiet {
            println!("  source ({}):", src.len());
        }
        for u in src {
            let inst = u.installed.as_deref().unwrap_or("(not installed)");
            println!("    {}  {} → {}", u.name, inst, u.candidate);
        }
    }
}

/// Prompt the user for a yes/no answer. Returns true if they say yes.
pub fn confirm_once(prompt: &str) -> bool {
    print!("{} [y/N] ", prompt);
    io::stdout().flush().ok();
    let mut line = String::new();
    io::stdin().read_line(&mut line).ok();
    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

pub fn dispatch_src(
    log: &Log,
    voidpkgs_override: Option<PathBuf>,
    cfg: Option<&Config>,
    cmd: SrcCmd,
) -> ExitCode {
    match cmd {
        // List doesn't need void-packages resolution.
        SrcCmd::List => return cmd_list(log),

        // Search needs resolution but we handle it inline.
        SrcCmd::Search { installed, term } => {
            let resolved = match resolve::resolve_voidpkgs(voidpkgs_override, cfg) {
                Ok(r) => r,
                Err(e) => {
                    log.error(e);
                    return ExitCode::from(2);
                }
            };
            return cmd_search(log, &resolved, installed, &term);
        }

        _ => {}
    }

    let resolved = match resolve::resolve_voidpkgs(voidpkgs_override, cfg) {
        Ok(r) => r,
        Err(e) => {
            log.error(e);
            return ExitCode::from(2);
        }
    };

    match cmd {
        SrcCmd::List | SrcCmd::Search { .. } => unreachable!(),

        SrcCmd::Build { local, pkgs } => {
            if pkgs.is_empty() {
                log.error("usage: vx src build <pkg> [pkg...]");
                return ExitCode::from(2);
            }
            let remote = !local;
            if remote {
                // Build from upstream worktree
                let wt = match git::ensure_upstream_worktree(log, &resolved.voidpkgs) {
                    Ok(p) => p,
                    Err(e) => {
                        log.error(e);
                        return ExitCode::from(1);
                    }
                };
                if let Err(e) = xbps_src::ensure_xbps_conf(log, &wt, resolved.use_nonfree) {
                    log.warn(format!("failed to ensure etc/conf: {e}"));
                }
                if let Err(e) =
                    xbps_src::overlay_local_srcpkgs(log, &resolved.voidpkgs, &wt, &pkgs)
                {
                    log.warn(format!("failed to overlay local srcpkgs: {e}"));
                }
                let env = xbps_src::build_env_for_worktree(&resolved);
                xbps_src::run_xbps_src_with_env(log, &wt, xbps_src::join_args("pkg", &pkgs), &env)
            } else {
                xbps_src::build(log, &resolved, &pkgs)
            }
        }

        SrcCmd::Clean { pkgs } => {
            if pkgs.is_empty() {
                log.error("usage: vx src clean <pkg> [pkg...]");
                return ExitCode::from(2);
            }
            xbps_src::clean(log, &resolved, &pkgs)
        }

        SrcCmd::Lint { pkgs } => {
            if pkgs.is_empty() {
                log.error("usage: vx src lint <pkg> [pkg...]");
                return ExitCode::from(2);
            }
            xbps_src::lint(log, &resolved, &pkgs)
        }

        SrcCmd::Add { yes, local, pkgs } => {
            if pkgs.is_empty() {
                log.error("usage: vx src add <pkg> [pkg...]");
                return ExitCode::from(2);
            }
            let remote = !local;
            xbps_src::src_up(log, &resolved, yes, remote, &pkgs)
        }

        SrcCmd::Rm { yes, pkgs } => {
            if pkgs.is_empty() {
                log.error("usage: vx src rm <pkg> [pkg...]");
                return ExitCode::from(2);
            }
            cmd_src_rm(log, cfg, yes, &pkgs)
        }

        SrcCmd::Up {
            dry_run,
            force,
            yes,
            local,
            pkgs,
        } => {
            let remote = !local;

            // Determine which packages to update.
            let targets: Option<Vec<String>> = if pkgs.is_empty() {
                None // plan_src_updates will load all managed
            } else {
                Some(pkgs.clone())
            };

            // Always plan first so we can show a summary and confirm.
            let updates = match plan::plan_src_updates(
                log,
                Some(resolved.voidpkgs.clone()),
                cfg,
                targets,
                force,
                remote,
            ) {
                Ok(v) => v,
                Err(e) => {
                    log.error(e);
                    return ExitCode::from(1);
                }
            };

            if updates.is_empty() {
                if !log.quiet {
                    println!("vx src: all packages up to date.");
                }
                return ExitCode::SUCCESS;
            }

            if !log.quiet {
                println!("source update plan ({}):", updates.len());
                for u in &updates {
                    let inst = u.installed.as_deref().unwrap_or("(not installed)");
                    println!("  {}  {} → {}", u.name, inst, u.candidate);
                }
            }

            if dry_run {
                return ExitCode::SUCCESS;
            }

            if !yes && !confirm_once("Proceed?") {
                log.info("aborted.");
                return ExitCode::SUCCESS;
            }

            let pkgs_to_update: Vec<String> = updates.iter().map(|u| u.name.clone()).collect();
            xbps_src::src_up(log, &resolved, yes, remote, &pkgs_to_update)
        }
    }
}

/// `vx src list` — show all tracked source packages with their installed version.
fn cmd_list(log: &Log) -> ExitCode {
    let managed = match managed::load_managed() {
        Ok(v) => v,
        Err(e) => {
            log.error(format!("failed to load managed list: {e}"));
            return ExitCode::from(1);
        }
    };

    if managed.is_empty() {
        if !log.quiet {
            println!("no source packages tracked. use `vx src add <pkg>` to start.");
        }
        return ExitCode::SUCCESS;
    }

    if !log.quiet {
        println!("tracked source packages ({}):", managed.len());
    }

    for pkg in &managed {
        // Try to get installed version via xbps-query.
        let version = xbps_query_pkgver(pkg).unwrap_or_else(|| "(not installed)".to_string());
        println!("  {:<30} {}", pkg, version);
    }

    ExitCode::SUCCESS
}

/// `vx src rm` — remove packages from system and untrack them.
fn cmd_src_rm(log: &Log, _cfg: Option<&Config>, yes: bool, pkgs: &[String]) -> ExitCode {
    // Confirm before removing.
    if !yes {
        println!("will remove and untrack:");
        for p in pkgs {
            println!("  {p}");
        }
        if !confirm_once("Proceed?") {
            log.info("aborted.");
            return ExitCode::SUCCESS;
        }
    }

    // xbps-remove
    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-remove");
    if yes {
        cmd.arg("-y");
    }
    cmd.args(pkgs);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    match cmd.status() {
        Ok(status) => {
            let code = status.code().unwrap_or(1) as u8;
            if code != 0 {
                return ExitCode::from(code);
            }
        }
        Err(e) => {
            log.error(format!("failed to run sudo xbps-remove: {e}"));
            return ExitCode::from(1);
        }
    }

    // Untrack from managed list.
    if let Err(e) = managed::remove_managed(&pkgs.to_vec()) {
        log.warn(format!("removed packages but failed to update managed list: {e}"));
    } else if log.verbose && !log.quiet {
        log.exec(format!("untracked: {}", pkgs.join(", ")));
    }

    ExitCode::SUCCESS
}

fn cmd_search(
    log: &Log,
    res: &resolve::SrcResolved,
    installed_only: bool,
    term: &str,
) -> ExitCode {
    let srcpkgs = res.voidpkgs.join("srcpkgs");
    if !srcpkgs.is_dir() {
        log.error(format!(
            "srcpkgs directory not found: {}",
            srcpkgs.display()
        ));
        return ExitCode::from(2);
    }

    let term_lower = term.to_lowercase();
    let mut matches: Vec<String> = Vec::new();

    let rd = match std::fs::read_dir(&srcpkgs) {
        Ok(r) => r,
        Err(e) => {
            log.error(format!("failed to read {}: {e}", srcpkgs.display()));
            return ExitCode::from(1);
        }
    };

    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.to_lowercase().contains(&term_lower) {
            continue;
        }
        if !entry.path().join("template").is_file() {
            continue;
        }
        if installed_only {
            if xbps_query_pkgver(&name).is_none() {
                continue;
            }
        }
        matches.push(name);
    }

    matches.sort();

    if matches.is_empty() {
        if !log.quiet {
            println!("no srcpkgs matching '{term}'");
        }
        return ExitCode::SUCCESS;
    }

    for m in &matches {
        let inst = if installed_only {
            String::new()
        } else {
            xbps_query_pkgver(m)
                .map(|v| format!("  [installed: {v}]"))
                .unwrap_or_default()
        };
        println!("{m}{inst}");
    }

    ExitCode::SUCCESS
}

fn xbps_query_pkgver(pkg: &str) -> Option<String> {
    let out = Command::new("xbps-query")
        .args(["-p", "pkgver", pkg])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    } else {
        None
    }
}
