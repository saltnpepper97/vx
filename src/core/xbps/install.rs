// Author Dustin Pilgrim
// License: MIT

use crate::{config::Config, log::Log, managed};
use std::process::{Command, ExitCode, Stdio};

use super::query;

pub fn add(log: &Log, _cfg: Option<&Config>, yes: bool, pkgs: &[String]) -> ExitCode {
    if pkgs.is_empty() {
        log.error("usage: vx add <pkg> [pkg...]");
        return ExitCode::from(2);
    }

    let mut to_install = Vec::new();
    for p in pkgs {
        match query::is_installed("xbps-query", p) {
            Ok(true) => log.warn(format!("package '{}' already installed.", p)),
            Ok(false) => to_install.push(p.clone()),
            Err(e) => {
                log.error(e);
                return ExitCode::from(1);
            }
        }
    }

    if to_install.is_empty() {
        log.info("nothing to do.");
        return ExitCode::SUCCESS;
    }

    run_install_cmd(log, &["-S"], &to_install, yes)
}

pub fn rm(log: &Log, _cfg: Option<&Config>, yes: bool, pkgs: &[String]) -> ExitCode {
    if pkgs.is_empty() {
        log.error("usage: vx rm <pkg> [pkg...]");
        return ExitCode::from(2);
    }

    let mut to_remove = Vec::new();
    for p in pkgs {
        match query::is_installed("xbps-query", p) {
            Ok(true) => to_remove.push(p.clone()),
            Ok(false) => log.warn(format!("package '{}' not installed.", p)),
            Err(e) => {
                log.error(e);
                return ExitCode::from(1);
            }
        }
    }

    if to_remove.is_empty() {
        log.info("nothing to do.");
        return ExitCode::SUCCESS;
    }

    // Determine which of these are also tracked as vx-managed src packages.
    // Non-fatal: removal should still work even if the manifest is missing/broken.
    let managed_list = match managed::load_managed() {
        Ok(v) => v,
        Err(e) => {
            log.warn(format!("failed to read managed-src list: {e}"));
            Vec::new()
        }
    };

    let mut to_untrack: Vec<String> = Vec::new();
    if !managed_list.is_empty() {
        for p in &to_remove {
            if managed_list.iter().any(|m| m == p) {
                to_untrack.push(p.clone());
            }
        }
    }

    // Remove packages first. Only untrack if removal succeeds.
    let code = run_remove_cmd(log, &[], &to_remove, yes);
    if code != ExitCode::SUCCESS {
        return code;
    }

    // New behavior:
    // If you removed a package that vx was also tracking as a source pkg,
    // automatically untrack it too (no prompt).
    if !to_untrack.is_empty() {
        if let Err(e) = managed::remove_managed(&to_untrack) {
            log.warn(format!("failed to update managed-src list: {e}"));
        } else if !log.quiet {
            if to_untrack.len() == 1 {
                log.info(format!("untracked source package '{}'.", to_untrack[0]));
            } else {
                log.info(format!("untracked {} source packages.", to_untrack.len()));
            }
        }
    }

    ExitCode::SUCCESS
}

pub fn up_with_yes(log: &Log, _cfg: Option<&Config>, yes: bool) -> ExitCode {
    run_install_cmd(log, &["-Su"], &[], yes)
}

fn run_install_cmd(log: &Log, opts: &[&str], args: &[String], yes: bool) -> ExitCode {
    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-install");
    cmd.args(opts);
    if yes {
        cmd.arg("-y");
    }
    cmd.args(args);

    if log.verbose && !log.quiet {
        let mut s = String::from("sudo xbps-install");
        for o in opts {
            s.push(' ');
            s.push_str(o);
        }
        if yes {
            s.push_str(" -y");
        }
        for a in args {
            s.push(' ');
            s.push_str(a);
        }
        log.exec(s);
    }

    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    match cmd.status() {
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            log.error(format!("failed to run xbps-install: {e}"));
            ExitCode::from(1)
        }
    }
}

fn run_remove_cmd(log: &Log, opts: &[&str], args: &[String], yes: bool) -> ExitCode {
    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-remove");
    cmd.args(opts);
    if yes {
        cmd.arg("-y");
    }
    cmd.args(args);

    if log.verbose && !log.quiet {
        let mut s = String::from("sudo xbps-remove");
        for o in opts {
            s.push(' ');
            s.push_str(o);
        }
        if yes {
            s.push_str(" -y");
        }
        for a in args {
            s.push(' ');
            s.push_str(a);
        }
        log.exec(s);
    }

    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    match cmd.status() {
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            log.error(format!("failed to run xbps-remove: {e}"));
            ExitCode::from(1)
        }
    }
}
