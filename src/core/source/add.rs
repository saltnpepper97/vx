// Author Dustin Pilgrim
// License: MIT

use crate::log::Log;
use std::process::{Command, ExitCode, Stdio};

use super::resolve::SrcResolved;

pub fn add_from_local_repo(
    log: &Log,
    res: &SrcResolved,
    force: bool,
    yes: bool,
    pkgs: &[String],
) -> ExitCode {
    if pkgs.is_empty() {
        log.error("usage: vx src add <pkg> [pkg...]");
        return ExitCode::from(2);
    }

    let base = res.voidpkgs.join(&res.local_repo_rel);
    if !base.exists() {
        log.error(format!(
            "local repo not found at {} (build packages first)",
            base.display()
        ));
        return ExitCode::from(2);
    }

    let mut to_install: Vec<String> = Vec::new();
    if force {
        to_install.extend_from_slice(pkgs);
    } else {
        for p in pkgs {
            match is_installed_system(p) {
                Ok(true) => log.warn(format!("package '{}' already installed.", p)),
                Ok(false) => to_install.push(p.clone()),
                Err(e) => {
                    log.error(e);
                    return ExitCode::from(1);
                }
            }
        }
    }

    if to_install.is_empty() {
        log.info("nothing to do.");
        return ExitCode::SUCCESS;
    }

    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-install");
    cmd.arg("-R").arg(&base);

    let nonfree = base.join("nonfree");
    if res.use_nonfree && nonfree.is_dir() {
        cmd.arg("-R").arg(&nonfree);
    }

    if force {
        cmd.arg("-f");
    }
    if yes {
        cmd.arg("-y");
    }

    cmd.args(&to_install);

    if log.verbose && !log.quiet {
        let mut s = format!("sudo xbps-install -R {}", base.display());
        if res.use_nonfree && nonfree.is_dir() {
            s.push_str(&format!(" -R {}", nonfree.display()));
        }
        if force {
            s.push_str(" -f");
        }
        if yes {
            s.push_str(" -y");
        }
        for p in &to_install {
            s.push(' ');
            s.push_str(p);
        }
        log.exec(s);
    }

    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    match cmd.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(e) => {
            log.error(format!("failed to run sudo xbps-install: {e}"));
            ExitCode::from(1)
        }
    }
}

fn is_installed_system(pkg: &str) -> Result<bool, String> {
    let status = Command::new("xbps-query")
        .arg("-p")
        .arg("pkgver")
        .arg(pkg)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("failed to run xbps-query: {e}"))?;

    Ok(status.success())
}

