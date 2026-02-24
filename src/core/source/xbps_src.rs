// Author Dustin Pilgrim
// License: MIT

use crate::{log::Log, managed};
use std::{
    ffi::OsString,
    fs,
    path::Path,
    process::{Command, ExitCode, Stdio},
};

use super::add;
use super::git;
use super::resolve::SrcResolved;

pub fn build(log: &Log, res: &SrcResolved, pkgs: &[String]) -> ExitCode {
    run_xbps_src(log, &res.voidpkgs, join_args("pkg", pkgs))
}

pub fn clean(log: &Log, res: &SrcResolved, pkgs: &[String]) -> ExitCode {
    run_xbps_src(log, &res.voidpkgs, join_args("clean", pkgs))
}

pub fn lint(log: &Log, res: &SrcResolved, pkgs: &[String]) -> ExitCode {
    run_xbps_src(log, &res.voidpkgs, join_args("lint", pkgs))
}

/// Build + install source packages, then track them in the managed list.
///
/// - remote=true (default): builds from upstream/master via git worktree.
///   Does not touch your local branch.
/// - remote=false (--local): builds from your local void-packages checkout.
pub fn src_up(log: &Log, res: &SrcResolved, yes: bool, remote: bool, pkgs: &[String]) -> ExitCode {
    if pkgs.is_empty() {
        log.error("no packages specified");
        return ExitCode::from(2);
    }

    let (dir, env) = if remote {
        let wt = match git::ensure_upstream_worktree(log, &res.voidpkgs) {
            Ok(p) => p,
            Err(e) => {
                log.error(e);
                return ExitCode::from(1);
            }
        };

        if let Err(e) = ensure_xbps_conf(log, &wt, res.use_nonfree) {
            log.warn(format!("failed to ensure etc/conf in worktree: {e}"));
        }

        if let Err(e) = overlay_local_srcpkgs(log, &res.voidpkgs, &wt, pkgs) {
            log.warn(format!(
                "failed to overlay local srcpkgs into upstream worktree: {e}"
            ));
        }

        (wt, build_env_for_worktree(res))
    } else {
        if let Err(e) = ensure_xbps_conf(log, &res.voidpkgs, res.use_nonfree) {
            log.warn(format!("failed to ensure etc/conf in local repo: {e}"));
        }
        (res.voidpkgs.clone(), Vec::new())
    };

    let c = run_xbps_src_with_env(log, &dir, join_args("clean", pkgs), &env);
    if c != ExitCode::SUCCESS {
        return c;
    }

    let c = run_xbps_src_with_env(log, &dir, join_args("pkg", pkgs), &env);
    if c != ExitCode::SUCCESS {
        return c;
    }

    let c = add::add_from_local_repo(log, res, true, yes, pkgs);

    if c == ExitCode::SUCCESS {
        if let Err(e) = managed::add_managed(&pkgs.to_vec()) {
            log.warn(format!("failed to update managed list: {e}"));
        }
    }

    c
}

pub fn join_args(sub: &str, pkgs: &[String]) -> Vec<OsString> {
    let mut out = Vec::with_capacity(1 + pkgs.len());
    out.push(OsString::from(sub));
    out.extend(pkgs.iter().cloned().map(OsString::from));
    out
}

fn run_xbps_src(log: &Log, voidpkgs: &Path, args: Vec<OsString>) -> ExitCode {
    run_xbps_src_with_env(log, voidpkgs, args, &[])
}

pub fn run_xbps_src_with_env(
    log: &Log,
    voidpkgs: &Path,
    args: Vec<OsString>,
    env: &[(String, String)],
) -> ExitCode {
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
        if !env.is_empty() {
            let mut pre = String::new();
            for (k, v) in env {
                pre.push_str(&format!("{k}={v} "));
            }
            log.exec(format!("(cd {}) && {pre}{s}", voidpkgs.display()));
        } else {
            log.exec(format!("(cd {}) && {}", voidpkgs.display(), s));
        }
    }

    let mut cmd = Command::new("./xbps-src");
    cmd.current_dir(voidpkgs)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    for (k, v) in env {
        cmd.env(k, v);
    }

    match cmd.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(e) => {
            log.error(format!("failed to run ./xbps-src: {e}"));
            ExitCode::from(1)
        }
    }
}

/// Ensure `etc/conf` contains XBPS_ALLOW_RESTRICTED=yes when allow_restricted=true.
pub fn ensure_xbps_conf(log: &Log, voidpkgs: &Path, allow_restricted: bool) -> Result<(), String> {
    if !allow_restricted {
        return Ok(());
    }

    let etc_dir = voidpkgs.join("etc");
    let conf = etc_dir.join("conf");

    fs::create_dir_all(&etc_dir)
        .map_err(|e| format!("failed to create {}: {e}", etc_dir.display()))?;

    let mut needs_write = true;
    if conf.is_file() {
        let text = fs::read_to_string(&conf)
            .map_err(|e| format!("failed to read {}: {e}", conf.display()))?;
        if text.lines().any(|l| l.trim() == "XBPS_ALLOW_RESTRICTED=yes") {
            needs_write = false;
        }
    }

    if needs_write {
        if log.verbose && !log.quiet {
            log.exec(format!("write {}", conf.display()));
        }
        let mut out = String::new();
        if conf.is_file() {
            out.push_str(
                &fs::read_to_string(&conf)
                    .map_err(|e| format!("failed to read {}: {e}", conf.display()))?,
            );
            if !out.ends_with('\n') {
                out.push('\n');
            }
        }
        out.push_str("XBPS_ALLOW_RESTRICTED=yes\n");
        fs::write(&conf, out).map_err(|e| format!("failed to write {}: {e}", conf.display()))?;
    }

    Ok(())
}

pub fn build_env_for_worktree(res: &SrcResolved) -> Vec<(String, String)> {
    let hostdir = res
        .voidpkgs
        .join(&res.local_repo_rel)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| res.voidpkgs.join("hostdir"));

    vec![
        (
            "XBPS_HOSTDIR".to_string(),
            hostdir.to_string_lossy().to_string(),
        ),
        (
            "XBPS_DISTDIR".to_string(),
            res.voidpkgs.join("distfiles").to_string_lossy().to_string(),
        ),
    ]
}

/// Overlay local `srcpkgs/<pkg>` into an upstream worktree.
///
/// Only overlays when:
/// - upstream doesn't have the package (fork-only), OR
/// - local has a `.vx-overlay` marker file (explicit override).
pub fn overlay_local_srcpkgs(
    log: &Log,
    local_repo: &Path,
    worktree: &Path,
    pkgs: &[String],
) -> Result<(), String> {
    for pkg in pkgs {
        let pkg = pkg.trim();
        if pkg.is_empty() {
            continue;
        }

        let local_dir = local_repo.join("srcpkgs").join(pkg);
        if !local_dir.is_dir() {
            continue;
        }

        let marker = local_dir.join(".vx-overlay");
        let upstream_has = git::upstream_has_template(local_repo, pkg);

        let do_overlay = marker.is_file() || !upstream_has;

        if !do_overlay {
            continue;
        }

        let wt_dir = worktree.join("srcpkgs").join(pkg);

        if wt_dir.exists() {
            fs::remove_dir_all(&wt_dir)
                .map_err(|e| format!("failed to remove {}: {e}", wt_dir.display()))?;
        }

        if log.verbose && !log.quiet {
            let why = if marker.is_file() { "marker .vx-overlay" } else { "fork-only" };
            log.exec(format!(
                "overlay ({why}): {} -> {}",
                local_dir.display(),
                wt_dir.display()
            ));
        }

        copy_dir_all(&local_dir, &wt_dir)?;
    }

    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst)
        .map_err(|e| format!("failed to create dir {}: {e}", dst.display()))?;

    for entry in fs::read_dir(src)
        .map_err(|e| format!("failed to read dir {}: {e}", src.display()))?
    {
        let entry = entry.map_err(|e| format!("read_dir entry failed: {e}"))?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        let meta = entry
            .metadata()
            .map_err(|e| format!("failed to stat {}: {e}", path.display()))?;

        if meta.is_dir() {
            copy_dir_all(&path, &target)?;
        } else if meta.is_file() {
            fs::copy(&path, &target).map_err(|e| {
                format!("failed to copy {} -> {}: {e}", path.display(), target.display())
            })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&target, fs::Permissions::from_mode(meta.permissions().mode()));
            }
        }
    }

    Ok(())
}
