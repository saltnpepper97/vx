// Author Dustin Pilgrim
// License: MIT

use crate::log::Log;
use std::{
    path::Path,
    process::{Command, Stdio},
};

pub fn sync_voidpkgs(log: &Log, voidpkgs: &Path) -> Result<(), String> {
    let git_dir = voidpkgs.join(".git");
    if !git_dir.exists() {
        return Err(format!(
            "void-packages at {} is not a git repo (missing .git); cannot sync",
            voidpkgs.display()
        ));
    }

    // Refuse if dirty
    let out = Command::new("git")
        .current_dir(voidpkgs)
        .args(["status", "--porcelain"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("failed to run git status: {e}"))?;

    if !out.status.success() {
        return Err(format!("git status failed in {}", voidpkgs.display()));
    }

    if !out.stdout.is_empty() {
        return Err(format!(
            "void-packages repo is dirty at {} (uncommitted changes); refusing to sync",
            voidpkgs.display()
        ));
    }

    // Ensure upstream exists
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
            "(cd {}) && git pull upstream master",
            voidpkgs.display()
        ));
    }

    let mut cmd = Command::new("git");
    cmd.current_dir(voidpkgs)
        .args(["pull", "upstream", "master"])
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
        .map_err(|e| format!("failed to run git pull: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "git pull upstream master failed in {}",
            voidpkgs.display()
        ))
    }
}

