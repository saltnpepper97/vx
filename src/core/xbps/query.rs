// Author Dustin Pilgrim
// License: MIT

use crate::{config::Config, log::Log};
use std::process::{Command, ExitCode, Stdio};

pub fn search(log: &Log, _cfg: Option<&Config>, installed: bool, term: &[String]) -> ExitCode {
    if term.is_empty() {
        log.error("usage: vx search <term>");
        return ExitCode::from(2);
    }

    let needle = term.join(" ");
    let opt = if installed { "-s" } else { "-Rs" };
    run_query_cmd(log, "xbps-query", &[opt, &needle])
}

pub fn info(log: &Log, _cfg: Option<&Config>, pkg: &str) -> ExitCode {
    if pkg.trim().is_empty() {
        log.error("usage: vx info <pkg>");
        return ExitCode::from(2);
    }
    run_query_cmd(log, "xbps-query", &["-R", pkg])
}

pub fn files(log: &Log, _cfg: Option<&Config>, pkg: &str) -> ExitCode {
    if pkg.trim().is_empty() {
        log.error("usage: vx files <pkg>");
        return ExitCode::from(2);
    }
    run_query_cmd(log, "xbps-query", &["-f", pkg])
}

pub fn provides(log: &Log, _cfg: Option<&Config>, path: &str) -> ExitCode {
    if path.trim().is_empty() {
        log.error("usage: vx provides <path>");
        return ExitCode::from(2);
    }
    run_query_cmd(log, "xbps-query", &["-o", path])
}

pub fn is_installed(xbps_query: &str, pkg: &str) -> Result<bool, String> {
    let status = Command::new(xbps_query)
        .arg("-p")
        .arg("pkgver")
        .arg(pkg)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("failed to run {xbps_query}: {e}"))?;

    Ok(status.success())
}

pub fn installed_pkgver(pkg: &str) -> Result<Option<String>, String> {
    let out = Command::new("xbps-query")
        .arg("-p")
        .arg("pkgver")
        .arg(pkg)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("failed to run xbps-query: {e}"))?;

    if !out.status.success() {
        return Ok(None);
    }

    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s))
    }
}

fn run_query_cmd(log: &Log, tool: &str, args: &[&str]) -> ExitCode {
    let mut cmd = Command::new(tool);
    cmd.args(args);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    if log.verbose && !log.quiet {
        let mut s = String::new();
        s.push_str(tool);
        for a in args {
            s.push(' ');
            s.push_str(a);
        }
        log.exec(s);
    }

    match cmd.status() {
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            log.error(format!("failed to run {tool}: {e}"));
            ExitCode::from(1)
        }
    }
}

