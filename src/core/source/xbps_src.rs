// Author Dustin Pilgrim
// License: MIT

use crate::{log::Log, managed};
use std::{
    ffi::OsString,
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
pub fn src_up(log: &Log, res: &SrcResolved, yes: bool, remote: bool, pkgs: &[String]) -> ExitCode {
    let (dir, env) = if remote {
        let wt = match git::ensure_upstream_worktree(log, &res.voidpkgs) {
            Ok(p) => p,
            Err(e) => {
                log.error(e);
                return ExitCode::from(1);
            }
        };
        (wt, build_env_for_worktree(res))
    } else {
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

/// Compute env vars to ensure worktree builds write into the main repo's hostdir/distfiles.
///
/// This keeps `vx src add` / install-from-local-repo working unchanged.
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

