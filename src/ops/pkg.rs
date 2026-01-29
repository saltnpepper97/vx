// Author Dustin Pilgrim
// License: MIT

use crate::{config::Config, log::Log};
use std::{
    env,
    fs,
    path::PathBuf,
    process::{Command, ExitCode, Stdio},
};

pub fn pkg_new(
    log: &Log,
    voidpkgs_override: Option<PathBuf>,
    cfg: Option<&Config>,
    name: &str,
) -> ExitCode {
    let voidpkgs = match resolve_voidpkgs_path(voidpkgs_override, cfg) {
        Ok(p) => p,
        Err(e) => {
            log.error(e);
            return ExitCode::from(2);
        }
    };

    let name = name.trim();
    if name.is_empty() {
        log.error("usage: vx pkg new <name>");
        return ExitCode::from(2);
    }

    if !voidpkgs.join("xbps-src").is_file() {
        log.error(format!(
            "not a void-packages directory (missing ./xbps-src): {}",
            voidpkgs.display()
        ));
        return ExitCode::from(2);
    }

    if log.verbose && !log.quiet {
        log.exec(format!("(cd {}) && xnew {}", voidpkgs.display(), name));
    }

    let mut cmd = Command::new("xnew");
    cmd.arg(name);
    cmd.current_dir(&voidpkgs);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    match cmd.status() {
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            log.error(format!(
                "failed to run xnew: {e}\n\
                 hint: install xtools (package name: xtools) to get `xnew`."
            ));
            ExitCode::from(1)
        }
    }
}

/// vx pkg <name> --gensum
///
/// Behavior:
/// - reads template before
/// - runs `xgensum -i` (plus optional flags)
/// - reads template after
/// - if unchanged -> prints "checksum unchanged (same version)"
/// - else -> "updated checksum(s) in template"
///
/// We delegate to xtools xgensum because it correctly understands Void templates,
/// multiple distfiles, hostdir layout, arch selection, and fetch rules.
pub fn pkg_gensum(
    log: &Log,
    voidpkgs_override: Option<PathBuf>,
    cfg: Option<&Config>,
    pkg: &str,
    force: bool,
    content: bool,
    arch: Option<&str>,
    hostdir: Option<&PathBuf>,
) -> ExitCode {
    let voidpkgs = match resolve_voidpkgs_path(voidpkgs_override, cfg) {
        Ok(p) => p,
        Err(e) => {
            log.error(e);
            return ExitCode::from(2);
        }
    };

    let pkg = pkg.trim();
    if pkg.is_empty() {
        log.error("usage: vx pkg <name> --gensum");
        return ExitCode::from(2);
    }

    if !voidpkgs.join("xbps-src").is_file() {
        log.error(format!(
            "not a void-packages directory (missing ./xbps-src): {}",
            voidpkgs.display()
        ));
        return ExitCode::from(2);
    }

    let tpl = voidpkgs.join("srcpkgs").join(pkg).join("template");
    if !tpl.is_file() {
        log.error(format!("template not found: {}", tpl.display()));
        return ExitCode::from(2);
    }

    let before = match fs::read_to_string(&tpl) {
        Ok(s) => s,
        Err(e) => {
            log.error(format!("failed to read {}: {e}", tpl.display()));
            return ExitCode::from(1);
        }
    };

    // Build xgensum args:
    // - We always want in-place update when it changes, so we always pass -i.
    let mut args: Vec<String> = Vec::new();
    args.push("-i".to_string());

    if force {
        args.push("-f".to_string());
    }
    if content {
        args.push("-c".to_string());
    }
    if let Some(a) = arch {
        if !a.trim().is_empty() {
            args.push("-a".to_string());
            args.push(a.trim().to_string());
        }
    }
    if let Some(h) = hostdir {
        if !h.as_os_str().is_empty() {
            args.push("-H".to_string());
            args.push(h.to_string_lossy().to_string());
        }
    }

    // xgensum accepts templates...; we pass the package name (it resolves srcpkgs/<pkg>/template)
    args.push(pkg.to_string());

    if log.verbose && !log.quiet {
        let mut s = format!("(cd {}) && xgensum", voidpkgs.display());
        for a in &args {
            s.push(' ');
            s.push_str(a);
        }
        log.exec(s);
    }

    let mut cmd = Command::new("xgensum");
    cmd.args(&args);
    cmd.current_dir(&voidpkgs);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let status = match cmd.status() {
        Ok(s) => s,
        Err(e) => {
            log.error(format!(
                "failed to run xgensum: {e}\n\
                 hint: install xtools (package name: xtools) to get `xgensum`."
            ));
            return ExitCode::from(1);
        }
    };

    if !status.success() {
        return ExitCode::from(status.code().unwrap_or(1) as u8);
    }

    let after = match fs::read_to_string(&tpl) {
        Ok(s) => s,
        Err(e) => {
            log.error(format!("failed to read {}: {e}", tpl.display()));
            return ExitCode::from(1);
        }
    };

    if before == after {
        log.info("checksum unchanged (same distfile/version).");
        return ExitCode::SUCCESS;
    }

    log.info("updated checksum(s) in template.");
    ExitCode::SUCCESS
}

fn resolve_voidpkgs_path(
    voidpkgs_override: Option<PathBuf>,
    cfg: Option<&Config>,
) -> Result<PathBuf, String> {
    // 1) CLI override
    if let Some(p) = voidpkgs_override {
        if !p.as_os_str().is_empty() {
            return Ok(p);
        }
    }

    // 2) env var
    if let Ok(v) = env::var("VX_VOIDPKGS") {
        let p = PathBuf::from(v);
        if !p.as_os_str().is_empty() {
            return Ok(p);
        }
    }

    // 3) config (Option<PathBuf>)
    if let Some(c) = cfg {
        if let Some(p) = &c.void_packages_path {
            if !p.as_os_str().is_empty() {
                return Ok(p.clone());
            }
        }
    }

    Err(
        "vx pkg requires a void-packages path.\n\
         Provide one of:\n\
         - --voidpkgs /path/to/void-packages\n\
         - VX_VOIDPKGS=/path/to/void-packages\n\
         - ~/.config/vx/vx.rune with void_packages.path\n"
            .to_string(),
    )
}

