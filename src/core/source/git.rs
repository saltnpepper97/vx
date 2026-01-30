// Author Dustin Pilgrim
// License: MIT

use crate::{cache, log::Log};
use std::{
    path::Path,
    process::{Command, Stdio},
};

const UPSTREAM_REF: &str = "upstream/master";

/// Fetch upstream refs without modifying the current branch/working tree.
///
/// - Uses TTL caching (default 10m). Set VX_FRESH=1 to force.
/// - Does NOT merge/rebase your branch.
/// - Safe even if repo is dirty.
pub fn sync_voidpkgs(log: &Log, voidpkgs: &Path) -> Result<(), String> {
    let ttl = cache::sync_ttl_secs();
    let cache_key = format!("voidpkgs.fetch:{}", voidpkgs.display());

    let git_dir = voidpkgs.join(".git");
    if !git_dir.exists() {
        return Err(format!(
            "void-packages at {} is not a git repo (missing .git); cannot sync",
            voidpkgs.display()
        ));
    }

    if cache::is_fresh(&cache_key, ttl) {
        if log.verbose && !log.quiet {
            log.exec(format!(
                "cache hit: skip git fetch (ttl={}s); set VX_FRESH=1 to force",
                ttl
            ));
        }
        return Ok(());
    }

    let has_upstream = Command::new("git")
        .current_dir(voidpkgs)
        .args(["remote", "get-url", "upstream"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !has_upstream {
        return Err(format!(
            "void-packages repo has no 'upstream' remote.\n\
             vx expects 'upstream' to point at the official Void Linux repository.\n\n\
             To fix this, run:\n\
               cd {}\n\
               git remote add upstream https://github.com/void-linux/void-packages.git\n\n\
             Then re-run your vx command.",
            voidpkgs.display()
        ));
    }

    if log.verbose && !log.quiet {
        log.exec(format!(
            "(cd {}) && git fetch upstream master",
            voidpkgs.display()
        ));
    }

    let mut cmd = Command::new("git");
    cmd.current_dir(voidpkgs)
        .args(["fetch", "upstream", "master"])
        .stdin(Stdio::null());

    if log.verbose && !log.quiet {
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
    } else {
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
    }

    let status = cmd
        .status()
        .map_err(|e| format!("failed to run git fetch: {e}"))?;

    if status.success() {
        cache::mark(&cache_key);
        Ok(())
    } else {
        Err(format!(
            "git fetch upstream master failed in {}",
            voidpkgs.display()
        ))
    }
}

/// Read an upstream template without checking anything out.
///
/// Equivalent to:
///   git show upstream/master:srcpkgs/<pkg>/template
pub fn read_template_upstream(voidpkgs: &Path, pkg: &str) -> Result<String, String> {
    let pkg = pkg.trim();
    if pkg.is_empty() {
        return Err("empty package name".to_string());
    }

    let spec = format!("{UPSTREAM_REF}:srcpkgs/{pkg}/template");

    let out = Command::new("git")
        .current_dir(voidpkgs)
        .args(["show", &spec])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run git show: {e}"))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.is_empty() {
            return Err(format!("git show failed for {spec}"));
        }
        return Err(err);
    }

    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

