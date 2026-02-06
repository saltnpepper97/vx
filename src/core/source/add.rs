// Author Dustin Pilgrim
// License: MIT

use crate::log::Log;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
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

    // Filter out already-installed unless forcing.
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

    // Discover all local repo directories we might have produced packages into.
    // This includes:
    // - hostdir/binpkgs
    // - hostdir/binpkgs/nonfree
    // - hostdir/binpkgs/<subrepo> (e.g. hostdir/binpkgs/stasis)
    // - hostdir/binpkgs/<subrepo>/nonfree
    let repo_pool = match discover_local_repo_dirs(&base, res.use_nonfree) {
        Ok(v) => v,
        Err(e) => {
            log.error(e);
            return ExitCode::from(1);
        }
    };

    if repo_pool.is_empty() {
        log.error(format!(
            "no usable local repositories found under {} (missing *-repodata?)",
            base.display()
        ));
        return ExitCode::from(2);
    }

    // For each pkg, find a repo dir that actually contains the .xbps file.
    // This avoids stale repodata entries causing checksum failures.
    let mut plan: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    let mut missing: Vec<String> = Vec::new();

    for pkg in &to_install {
        match choose_repo_for_pkg(&repo_pool, pkg) {
            Some(repo) => {
                plan.entry(repo).or_default().push(pkg.clone());
            }
            None => missing.push(pkg.clone()),
        }
    }

    if !missing.is_empty() {
        log.error(format!(
            "package(s) not found in local repository pool: {}",
            missing.join(", ")
        ));
        if log.verbose && !log.quiet {
            log.exec("hint: ensure you built them and that their .xbps exists in hostdir/binpkgs/<repo>/".to_string());
        }
        return ExitCode::from(2);
    }

    // Install per-repo so we never accidentally resolve a pkg from the wrong local repo.
    for (repo_dir, pkgs_for_repo) in plan {
        let mut cmd = Command::new("sudo");
        cmd.arg("xbps-install");
        cmd.arg("-R").arg(&repo_dir);

        if force {
            cmd.arg("-f");
        }
        if yes {
            cmd.arg("-y");
        }
        cmd.args(&pkgs_for_repo);

        if log.verbose && !log.quiet {
            let mut s = format!("sudo xbps-install -R {}", repo_dir.display());
            if force {
                s.push_str(" -f");
            }
            if yes {
                s.push_str(" -y");
            }
            for p in &pkgs_for_repo {
                s.push(' ');
                s.push_str(p);
            }
            log.exec(s);
        }

        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        match cmd.status() {
            Ok(status) => {
                let code = status.code().unwrap_or(1) as u8;
                if code != 0 {
                    return ExitCode::from(code);
                }
            }
            Err(e) => {
                log.error(format!("failed to run sudo xbps-install: {e}"));
                return ExitCode::from(1);
            }
        }
    }

    ExitCode::SUCCESS
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

/// Discover local xbps repository directories under `base` (hostdir/binpkgs).
///
/// We consider a directory a repo if it contains an `*-repodata` file (e.g. x86_64-repodata).
fn discover_local_repo_dirs(base: &Path, use_nonfree: bool) -> Result<Vec<PathBuf>, String> {
    let mut out: Vec<PathBuf> = Vec::new();

    // base itself
    if is_repo_dir(base) {
        out.push(base.to_path_buf());
    }

    // base/nonfree
    let nonfree = base.join("nonfree");
    if use_nonfree && nonfree.is_dir() && is_repo_dir(&nonfree) {
        out.push(nonfree);
    }

    // base/<subrepo> and base/<subrepo>/nonfree
    for entry in fs::read_dir(base).map_err(|e| format!("failed to read {}: {e}", base.display()))?
    {
        let entry = entry.map_err(|e| format!("read_dir entry failed: {e}"))?;
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }

        if is_repo_dir(&p) {
            out.push(p.clone());
        }

        if use_nonfree {
            let nf = p.join("nonfree");
            if nf.is_dir() && is_repo_dir(&nf) {
                out.push(nf);
            }
        }
    }

    // Dedup while preserving order (simple O(n^2) fine for tiny lists)
    let mut dedup: Vec<PathBuf> = Vec::new();
    for p in out {
        if !dedup.iter().any(|x| x == &p) {
            dedup.push(p);
        }
    }

    Ok(dedup)
}

fn is_repo_dir(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    // Void repo metadata: e.g. x86_64-repodata
    match fs::read_dir(dir) {
        Ok(rd) => rd
            .flatten()
            .any(|e| e.file_name().to_string_lossy().ends_with("-repodata")),
        Err(_) => false,
    }
}

/// Choose a repo that *actually contains* an .xbps file for `pkg`.
///
/// This is stricter than “repodata claims it exists”, and avoids:
///   ERROR: <pkg>: failed to checksum: No such file or directory
fn choose_repo_for_pkg(repos: &[PathBuf], pkg: &str) -> Option<PathBuf> {
    // Prefer repos where the actual .xbps file exists.
    for r in repos {
        if repo_has_pkg_file(r, pkg) {
            return Some(r.clone());
        }
    }
    None
}

/// True if repo dir contains a file that looks like: <pkg>-*.xbps
fn repo_has_pkg_file(repo: &Path, pkg: &str) -> bool {
    let Ok(rd) = fs::read_dir(repo) else {
        return false;
    };

    let prefix = format!("{pkg}-");
    for entry in rd.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) && name.ends_with(".xbps") {
            return true;
        }
    }
    false
}
