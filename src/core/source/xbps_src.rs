// Author Dustin Pilgrim
// License: MIT

use crate::{log::Log, managed};
use std::{
    ffi::OsString,
    path::Path,
    process::{Command, ExitCode, Stdio},
};

use super::add;
use super::resolve::SrcResolved;

pub fn build(log: &Log, res: &SrcResolved, pkgs: &[String]) -> ExitCode {
    if let Err(code) = need_pkgs(log, "vx src build", pkgs) {
        return code;
    }
    run_xbps_src(log, &res.voidpkgs, join_args("pkg", pkgs))
}

pub fn clean(log: &Log, res: &SrcResolved, pkgs: &[String]) -> ExitCode {
    if let Err(code) = need_pkgs(log, "vx src clean", pkgs) {
        return code;
    }
    run_xbps_src(log, &res.voidpkgs, join_args("clean", pkgs))
}

pub fn lint(log: &Log, res: &SrcResolved, pkgs: &[String]) -> ExitCode {
    if let Err(code) = need_pkgs(log, "vx src lint", pkgs) {
        return code;
    }
    run_xbps_src(log, &res.voidpkgs, join_args("lint", pkgs))
}

pub fn src_up(log: &Log, res: &SrcResolved, yes: bool, pkgs: &[String]) -> ExitCode {
    let c = run_xbps_src(log, &res.voidpkgs, join_args("clean", pkgs));
    if c != ExitCode::SUCCESS {
        return c;
    }

    let c = run_xbps_src(log, &res.voidpkgs, join_args("pkg", pkgs));
    if c != ExitCode::SUCCESS {
        return c;
    }

    let c = add::add_from_local_repo(log, res, true, yes, pkgs);

    if c == ExitCode::SUCCESS {
        if let Err(e) = managed::add_managed(&pkgs.to_vec()) {
            log.warn(format!("failed to update managed-src list: {e}"));
        }
    }

    c
}

fn need_pkgs(log: &Log, usage: &str, pkgs: &[String]) -> Result<(), ExitCode> {
    if pkgs.is_empty() {
        log.error(format!("usage: {usage} <pkg> [pkg...]"));
        Err(ExitCode::from(2))
    } else {
        Ok(())
    }
}

fn join_args(sub: &str, pkgs: &[String]) -> Vec<OsString> {
    let mut out = Vec::with_capacity(1 + pkgs.len());
    out.push(OsString::from(sub));
    out.extend(pkgs.iter().cloned().map(OsString::from));
    out
}

fn run_xbps_src(log: &Log, voidpkgs: &Path, args: Vec<OsString>) -> ExitCode {
    if !voidpkgs.join("xbps-src").is_file() {
        log.error(format!(
            "not a void-packages directory (missing ./xbps-src): {}",
            voidpkgs.display()
        ));
        return ExitCode::from(2);
    }

    if log.verbose && !log.quiet {
        let mut s = String::from("./xbps-src");
        for a in &args {
            s.push(' ');
            s.push_str(&a.to_string_lossy());
        }
        log.exec(format!("(cd {}) && {}", voidpkgs.display(), s));
    }

    match Command::new("./xbps-src")
        .current_dir(voidpkgs)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
    {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(e) => {
            log.error(format!("failed to run ./xbps-src: {e}"));
            ExitCode::from(1)
        }
    }
}

