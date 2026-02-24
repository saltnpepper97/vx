// Author Dustin Pilgrim
// License: MIT

use crate::{config::Config, log::Log};
use std::process::{Command, ExitCode, Stdio};

pub fn add(log: &Log, _cfg: Option<&Config>, yes: bool, pkgs: &[String]) -> ExitCode {
    if pkgs.is_empty() {
        log.error("usage: vx add <pkgs...>");
        return ExitCode::from(2);
    }

    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-install");
    if yes {
        cmd.arg("-y");
    }
    cmd.arg("-S");
    cmd.args(pkgs);

    run(log, cmd, "sudo xbps-install ...")
}

pub fn rm(
    log: &Log,
    _cfg: Option<&Config>,
    yes: bool,
    orphans: bool,
    pkgs: &[String],
) -> ExitCode {
    if pkgs.is_empty() && !orphans {
        log.error("usage: vx rm <pkgs...> [--orphans]");
        return ExitCode::from(2);
    }

    // 1) Remove requested packages (if any)
    if !pkgs.is_empty() {
        let mut cmd = Command::new("sudo");
        cmd.arg("xbps-remove");
        if yes {
            cmd.arg("-y");
        }
        // -R = remove deps that are no longer needed due to this removal (xbps semantics)
        cmd.arg("-R");
        cmd.args(pkgs);

        let code = run(log, cmd, "sudo xbps-remove ...");
        if code != ExitCode::SUCCESS {
            return code;
        }
    }

    // 2) Optional orphan cleanup pass
    if orphans {
        let mut cmd = Command::new("sudo");
        cmd.arg("xbps-remove");
        if yes {
            cmd.arg("-y");
        }
        cmd.arg("-o");

        return run(log, cmd, "sudo xbps-remove -o");
    }

    ExitCode::SUCCESS
}

pub fn up_with_yes(log: &Log, _cfg: Option<&Config>, yes: bool) -> ExitCode {
    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-install");
    if yes {
        cmd.arg("-y");
    }
    cmd.arg("-u");

    run(log, cmd, "sudo xbps-install -u")
}

fn run(log: &Log, mut cmd: Command, label: &str) -> ExitCode {
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    if log.verbose && !log.quiet {
        log.exec(label.to_string());
    }

    match cmd.status() {
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            log.error(format!("failed to run: {e}"));
            ExitCode::from(1)
        }
    }
}
