// Author Dustin Pilgrim
// License: MIT

use crate::{config::Config, log::Log};
use std::process::{Command, ExitCode, Stdio};

#[derive(Debug, Clone)]
pub struct SysUpdate {
    pub name: String,
    pub from: String,
    pub to: String,
}

pub fn search(log: &Log, cfg: Option<&Config>, installed: bool, term: &[String]) -> ExitCode {
    if term.is_empty() {
        log.error("usage: vx search <term>");
        return ExitCode::from(2);
    }

    let (_sudo, _install, query) = xbps_tools(cfg);

    let needle = term.join(" ");
    let opt = if installed { "-s" } else { "-Rs" };

    run_query_cmd(log, &query, &[opt, &needle])
}

pub fn info(log: &Log, cfg: Option<&Config>, pkg: &str) -> ExitCode {
    if pkg.trim().is_empty() {
        log.error("usage: vx info <pkg>");
        return ExitCode::from(2);
    }

    let (_sudo, _install, query) = xbps_tools(cfg);
    run_query_cmd(log, &query, &["-R", pkg])
}

pub fn files(log: &Log, cfg: Option<&Config>, pkg: &str) -> ExitCode {
    if pkg.trim().is_empty() {
        log.error("usage: vx files <pkg>");
        return ExitCode::from(2);
    }

    let (_sudo, _install, query) = xbps_tools(cfg);
    run_query_cmd(log, &query, &["-f", pkg])
}

pub fn provides(log: &Log, cfg: Option<&Config>, path: &str) -> ExitCode {
    if path.trim().is_empty() {
        log.error("usage: vx provides <path>");
        return ExitCode::from(2);
    }

    let (_sudo, _install, query) = xbps_tools(cfg);
    run_query_cmd(log, &query, &["-o", path])
}

pub fn add(log: &Log, cfg: Option<&Config>, yes: bool, pkgs: &[String]) -> ExitCode {
    if pkgs.is_empty() {
        log.error("usage: vx add <pkg> [pkg...]");
        return ExitCode::from(2);
    }

    let (sudo, install, query) = xbps_tools(cfg);

    let mut to_install = Vec::new();
    for p in pkgs {
        match is_installed(&query, p) {
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

    run_cmd(log, sudo, &install, &[], &to_install, yes)
}

pub fn rm(log: &Log, cfg: Option<&Config>, yes: bool, pkgs: &[String]) -> ExitCode {
    if pkgs.is_empty() {
        log.error("usage: vx rm <pkg> [pkg...]");
        return ExitCode::from(2);
    }

    let (sudo, _install, query) = xbps_tools(cfg);
    let remove = cfg
        .map(|c| c.xbps_remove.clone())
        .unwrap_or_else(|| "xbps-remove".to_string());

    let mut to_remove = Vec::new();
    for p in pkgs {
        match is_installed(&query, p) {
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

    run_cmd(log, sudo, &remove, &[], &to_remove, yes)
}

pub fn up_with_yes(log: &Log, cfg: Option<&Config>, yes: bool) -> ExitCode {
    let (sudo, install, _query) = xbps_tools(cfg);
    run_cmd(log, sudo, &install, &["-Su"], &[], yes)
}

/// Dry-run system update and parse versions.
/// Uses: xbps-install -Suvn
pub fn plan_system_updates(log: &Log, cfg: Option<&Config>) -> Result<Vec<SysUpdate>, String> {
    let (sudo, install, _query) = xbps_tools(cfg);

    let mut cmd = if sudo {
        let mut c = Command::new("sudo");
        c.arg(&install);
        c
    } else {
        Command::new(&install)
    };

    cmd.args(["-Suvn"]);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    log.exec(format!(
        "{}{} -Suvn",
        if sudo { "sudo " } else { "" },
        install
    ));

    let out = cmd
        .output()
        .map_err(|e| format!("failed to run {install} -Suvn: {e}"))?;

    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    Ok(parse_xbps_dry_run(&text))
}

fn xbps_tools(cfg: Option<&Config>) -> (bool, String, String) {
    let sudo = cfg.map(|c| c.xbps_sudo).unwrap_or(true);

    let install = cfg
        .map(|c| c.xbps_install.clone())
        .unwrap_or_else(|| "xbps-install".to_string());

    let query = cfg
        .map(|c| c.xbps_query.clone())
        .unwrap_or_else(|| "xbps-query".to_string());

    (sudo, install, query)
}

fn is_installed(xbps_query: &str, pkg: &str) -> Result<bool, String> {
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

fn run_cmd(log: &Log, sudo: bool, tool: &str, opts: &[&str], args: &[String], yes: bool) -> ExitCode {
    let mut cmd = if sudo {
        let mut c = Command::new("sudo");
        c.arg(tool);
        c
    } else {
        Command::new(tool)
    };

    cmd.args(opts);
    if yes {
        cmd.arg("-y");
    }
    cmd.args(args);

    if log.verbose && !log.quiet {
        let mut s = String::new();
        if sudo {
            s.push_str("sudo ");
        }
        s.push_str(tool);
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
            log.error(format!("failed to run {tool}: {e}"));
            ExitCode::from(1)
        }
    }
}

fn parse_xbps_dry_run(text: &str) -> Vec<SysUpdate> {
    let mut out = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some((a, b)) = line.split_once("->") {
            let mut lhs = a.trim();
            let mut rhs = b.trim();

            lhs = lhs.trim_start_matches(|c: char| c == '*' || c == '-' || c.is_whitespace());
            rhs = rhs.trim_start_matches(|c: char| c == '*' || c == '-' || c.is_whitespace());

            if lhs.is_empty() || rhs.is_empty() {
                continue;
            }

            let name = pkgname_from_pkgver(lhs).unwrap_or_else(|| "<pkg>".to_string());
            out.push(SysUpdate {
                name,
                from: lhs.to_string(),
                to: rhs.to_string(),
            });
            continue;
        }

        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 4 && cols[1] == "update" {
            out.push(SysUpdate {
                name: cols[0].to_string(),
                from: cols[2].to_string(),
                to: cols[3].to_string(),
            });
            continue;
        }
    }

    out
}

fn pkgname_from_pkgver(pkgver: &str) -> Option<String> {
    let (name, ver) = pkgver.rsplit_once('-')?;
    if ver.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        Some(name.to_string())
    } else {
        None
    }
}

