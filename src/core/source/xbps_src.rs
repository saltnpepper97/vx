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

/// Update source packages (clean+pkg), then install from the local repo.
///
/// Behavior:
/// - remote=false (default): build from your local void-packages checkout.
/// - remote=true: build from upstream/master via git worktree (does not mutate your branch),
///   but writes outputs into the main repo hostdir so installs work normally.
///
/// Remote automation:
/// - When remote=true, vx overlays local `srcpkgs/<pkg>` into upstream worktree ONLY when:
///     * upstream does not contain that package (fork-only), OR
///     * local contains `srcpkgs/<pkg>/.vx-overlay` marker.
/// - Also writes `etc/conf` in the build tree so restricted packages build automatically
///   when `use_nonfree=true`.
pub fn src_up(log: &Log, res: &SrcResolved, yes: bool, remote: bool, pkgs: &[String]) -> ExitCode {
    let (dir, env) = if remote {
        let wt = match git::ensure_upstream_worktree(log, &res.voidpkgs) {
            Ok(p) => p,
            Err(e) => {
                log.error(e);
                return ExitCode::from(1);
            }
        };

        // Ensure etc/conf has XBPS_ALLOW_RESTRICTED when nonfree enabled.
        if let Err(e) = ensure_xbps_conf(log, &wt, res.use_nonfree) {
            log.warn(format!("failed to ensure etc/conf in worktree: {e}"));
        }

        // Overlay fork-only (or explicitly marked) packages into worktree.
        if let Err(e) = overlay_local_srcpkgs(log, &res.voidpkgs, &wt, pkgs) {
            log.warn(format!("failed to overlay local srcpkgs into upstream worktree: {e}"));
        }

        (wt, build_env_for_worktree(res))
    } else {
        // Local builds: still ensure etc/conf for restricted if desired.
        if let Err(e) = ensure_xbps_conf(log, &res.voidpkgs, res.use_nonfree) {
            log.warn(format!("failed to ensure etc/conf in local repo: {e}"));
        }
        (res.voidpkgs.clone(), build_env_for_local(res))
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
    run_xbps_src_with_env(log, voidpkgs, args, &[])
}

/// Run xbps-src in a given directory, optionally with extra env vars.
/// `env` is a list of (key, value) pairs.
fn run_xbps_src_with_env(
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
                pre.push_str(k);
                pre.push('=');
                pre.push_str(v);
                pre.push(' ');
            }
            log.exec(format!("(cd {}) && {}{}", voidpkgs.display(), pre, s));
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

/// Ensure `etc/conf` in the given void-packages tree contains XBPS_ALLOW_RESTRICTED=yes
/// when allowed=true. This matches xbps-src's own error message expectation.
fn ensure_xbps_conf(log: &Log, voidpkgs: &Path, allow_restricted: bool) -> Result<(), String> {
    if !allow_restricted {
        return Ok(());
    }

    let etc_dir = voidpkgs.join("etc");
    let conf = etc_dir.join("conf");

    fs::create_dir_all(&etc_dir)
        .map_err(|e| format!("failed to create {}: {e}", etc_dir.display()))?;

    let mut needs_write = true;
    if conf.is_file() {
        let text =
            fs::read_to_string(&conf).map_err(|e| format!("failed to read {}: {e}", conf.display()))?;
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

/// Compute env vars to ensure worktree builds write into the main repo's hostdir/distfiles.
fn build_env_for_worktree(res: &SrcResolved) -> Vec<(String, String)> {
    let mut env = Vec::new();

    // local_repo_rel defaults to hostdir/binpkgs -> hostdir is parent
    let hostdir = res
        .voidpkgs
        .join(&res.local_repo_rel)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| res.voidpkgs.join("hostdir"));

    env.push((
        "XBPS_HOSTDIR".to_string(),
        hostdir.to_string_lossy().to_string(),
    ));

    // Share distfiles to avoid re-downloading in the worktree
    let dist = res.voidpkgs.join("distfiles");
    env.push((
        "XBPS_DISTDIR".to_string(),
        dist.to_string_lossy().to_string(),
    ));

    env
}

fn build_env_for_local(_res: &SrcResolved) -> Vec<(String, String)> {
    Vec::new()
}

/// Overlay local `srcpkgs/<pkg>` directories into an upstream worktree.
///
/// Rules:
/// - If upstream has srcpkgs/<pkg>/template, we DO NOT overlay by default (prevents stale fork copies).
/// - If upstream is missing it, we overlay (fork-only packages).
/// - If local contains `srcpkgs/<pkg>/.vx-overlay`, we overlay even if upstream has it (explicit override).
fn overlay_local_srcpkgs(
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

        // Overlay decision
        let do_overlay = if marker.is_file() {
            true
        } else {
            !upstream_has
        };

        if !do_overlay {
            continue;
        }

        let wt_dir = worktree.join("srcpkgs").join(pkg);

        if wt_dir.exists() {
            fs::remove_dir_all(&wt_dir)
                .map_err(|e| format!("failed to remove {}: {e}", wt_dir.display()))?;
        }

        if log.verbose && !log.quiet {
            let why = if marker.is_file() {
                "marker .vx-overlay"
            } else {
                "fork-only (missing upstream)"
            };
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

/// Recursively copy a directory.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst)
        .map_err(|e| format!("failed to create dir {}: {e}", dst.display()))?;

    for entry in fs::read_dir(src)
        .map_err(|e| format!("failed to read dir {}: {e}", src.display()))?
    {
        let entry = entry.map_err(|e| format!("read_dir entry failed: {e}"))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let target = dst.join(file_name);

        let meta = entry
            .metadata()
            .map_err(|e| format!("failed to stat {}: {e}", path.display()))?;

        if meta.is_dir() {
            copy_dir_all(&path, &target)?;
        } else if meta.is_file() {
            fs::copy(&path, &target).map_err(|e| {
                format!(
                    "failed to copy {} -> {}: {e}",
                    path.display(),
                    target.display()
                )
            })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = meta.permissions().mode();
                let _ = fs::set_permissions(&target, fs::Permissions::from_mode(mode));
            }
        } else {
            // ignore symlinks/special files
        }
    }

    Ok(())
}
