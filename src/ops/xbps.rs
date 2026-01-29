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

pub fn add(log: &Log, _cfg: Option<&Config>, yes: bool, pkgs: &[String]) -> ExitCode {
    if pkgs.is_empty() {
        log.error("usage: vx add <pkg> [pkg...]");
        return ExitCode::from(2);
    }

    let mut to_install = Vec::new();
    for p in pkgs {
        match is_installed("xbps-query", p) {
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
        match is_installed("xbps-query", p) {
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

    run_remove_cmd(log, &[], &to_remove, yes)
}

pub fn up_with_yes(log: &Log, _cfg: Option<&Config>, yes: bool) -> ExitCode {
    run_install_cmd(log, &["-Su"], &[], yes)
}

/// Dry-run system update and parse versions.
///
/// Uses: sudo xbps-install -Sun
///
/// NOTE: xbps may output either:
///  (A) a human table:
///      Name Action Version New version Download size
///  (B) a column-ish plan with pkgver first:
///      <pkgver> <action> <arch> <repo> ...
///
/// We parse BOTH.
///
/// IMPORTANT: We disable colors + strip ANSI so parsing doesn't silently break.
pub fn plan_system_updates(log: &Log, _cfg: Option<&Config>) -> Result<Vec<SysUpdate>, String> {
    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-install");
    cmd.args(["-Sun"]);
    cmd.env("XBPS_COLORS", "0"); // avoid ANSI output that can break parsing
    cmd.stdin(Stdio::inherit()); // allow sudo prompt
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    log.exec("sudo xbps-install -Sun".to_string());

    let out = cmd
        .output()
        .map_err(|e| format!("failed to run xbps-install -Sun: {e}"))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.is_empty() {
            return Err(format!(
                "xbps-install -Sun failed (exit={})",
                out.status.code().unwrap_or(1)
            ));
        }
        return Err(format!("xbps-install -Sun failed: {err}"));
    }

    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let text = strip_ansi(&text);

    let plan = parse_xbps_sun_plan(&text)?;

    // Refuse to silently claim "no updates" if output smelled like a plan but we parsed nothing.
    if plan.is_empty()
        && (text.contains("Name")
            && text.contains("Action")
            && text.contains("Version")
            && text.contains("New"))
    {
        return Err("failed to parse xbps -Sun output (format changed); refusing to report empty plan"
            .to_string());
    }

    Ok(plan)
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

fn installed_pkgver(pkg: &str) -> Result<Option<String>, String> {
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

/// Parse `xbps-install -Sun` output.
///
/// Supports:
///  A) table format:
///     Name Action Version New version Download size
///     firefox update 147.0_1 147.0.2_1 82MB
///
///  B) column format:
///     <pkgver> <action> <arch> <repo> ...
fn parse_xbps_sun_plan(text: &str) -> Result<Vec<SysUpdate>, String> {
    let mut out: Vec<SysUpdate> = Vec::new();

    let mut in_table = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            in_table = false;
            continue;
        }

        // ignore repo sync chatter + prompts
        if line.starts_with("[*]")
            || line.starts_with("=>")
            || line.starts_with("xbps-install:")
            || line.starts_with("Size to download:")
            || line.starts_with("Size required on disk:")
            || line.starts_with("Space available on disk:")
            || line.starts_with("Do you want to continue?")
            || line.starts_with("Aborting!")
        {
            continue;
        }

        // Detect the human table header
        if line.starts_with("Name")
            && line.contains("Action")
            && line.contains("Version")
            && line.contains("New")
        {
            in_table = true;
            continue;
        }

        // ------------------------
        // A) parse table rows
        // ------------------------
        if in_table {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 4 {
                continue;
            }

            let name = cols[0].to_string();
            let action = cols[1];

            if !matches!(action, "update" | "install" | "reinstall" | "downgrade") {
                continue;
            }

            let oldver = cols[2];
            let newver = cols[3];

            let from = format!("{name}-{oldver}");
            let to = format!("{name}-{newver}");

            out.push(SysUpdate { name, from, to });
            continue;
        }

        // ------------------------
        // B) parse column-ish rows
        // ------------------------
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 2 {
            continue;
        }

        let pkgver = cols[0];
        let action = cols[1];

        if !matches!(action, "update" | "install" | "reinstall" | "downgrade") {
            continue;
        }

        let name = match pkgname_from_pkgver(pkgver) {
            Some(n) => n,
            None => continue,
        };

        let from = match installed_pkgver(&name) {
            Ok(Some(v)) => v,
            Ok(None) => "<not installed>".to_string(),
            Err(e) => return Err(e),
        };

        out.push(SysUpdate {
            name,
            from,
            to: pkgver.to_string(),
        });
    }

    // de-dupe by name (keep last)
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name == b.name);

    Ok(out)
}

fn pkgname_from_pkgver(pkgver: &str) -> Option<String> {
    let (name, ver) = pkgver.rsplit_once('-')?;
    if ver.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        Some(name.to_string())
    } else {
        None
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' {
            if it.peek() == Some(&'[') {
                it.next(); // '['
                // consume until final letter
                while let Some(n) = it.next() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
                continue;
            }
        }
        out.push(c);
    }
    out
}

