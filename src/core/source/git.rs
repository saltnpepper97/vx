// Author Dustin Pilgrim
// License: MIT

use crate::{cache, log::Log};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const UPSTREAM_REF: &str = "upstream/master";

fn xdg_cache_home() -> PathBuf {
    if let Ok(v) = std::env::var("XDG_CACHE_HOME") {
        let p = PathBuf::from(v);
        if !p.as_os_str().is_empty() {
            return p;
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cache")
}

fn worktree_root_dir() -> PathBuf {
    xdg_cache_home().join("vx").join("worktrees")
}

fn stable_hash(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

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

/// Ensure we have a reusable worktree checked out at upstream/master, and return its path.
///
/// Behavior:
/// - Worktree lives in ~/.cache/vx/worktrees/<hash>/upstream-master
/// - If missing, we `git worktree add --detach` it.
/// - Each call then hard-resets it to upstream/master and cleans untracked files.
/// - This lets us build upstream templates without touching your current branch.
pub fn ensure_upstream_worktree(log: &Log, voidpkgs: &Path) -> Result<PathBuf, String> {
    // Ensure upstream ref is current (cached fetch)
    sync_voidpkgs(log, voidpkgs)?;

    let root = worktree_root_dir();
    fs::create_dir_all(&root).map_err(|e| format!("failed to create worktree dir: {e}"))?;

    let h = stable_hash(&voidpkgs.display().to_string());
    let repo_bucket = root.join(h);
    fs::create_dir_all(&repo_bucket)
        .map_err(|e| format!("failed to create worktree bucket: {e}"))?;

    let wt = repo_bucket.join("upstream-master");

    // If it doesn't exist, add it.
    if !wt.exists() {
        if log.verbose && !log.quiet {
            log.exec(format!(
                "(cd {}) && git worktree add --detach {} {}",
                voidpkgs.display(),
                wt.display(),
                UPSTREAM_REF
            ));
        }

        let out = Command::new("git")
            .current_dir(voidpkgs)
            .args([
                "worktree",
                "add",
                "--detach",
                wt.to_string_lossy().as_ref(),
                UPSTREAM_REF,
            ])
            .stdin(Stdio::null())
            .stdout(if log.verbose && !log.quiet {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            .stderr(if log.verbose && !log.quiet {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            .status()
            .map_err(|e| format!("failed to run git worktree add: {e}"))?;

        if !out.success() {
            return Err(format!(
                "git worktree add failed for {}",
                wt.display()
            ));
        }
    }

    // Make sure the worktree is exactly at upstream/master and clean.
    // (Detached worktree can be safely reset; it's vx-owned.)
    if log.verbose && !log.quiet {
        log.exec(format!(
            "(cd {}) && git reset --hard {}",
            wt.display(),
            UPSTREAM_REF
        ));
    }

    let st = Command::new("git")
        .current_dir(&wt)
        .args(["reset", "--hard", UPSTREAM_REF])
        .stdin(Stdio::null())
        .stdout(if log.verbose && !log.quiet {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .stderr(if log.verbose && !log.quiet {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .status()
        .map_err(|e| format!("failed to run git reset in worktree: {e}"))?;

    if !st.success() {
        return Err(format!(
            "failed to reset worktree to {} at {}",
            UPSTREAM_REF,
            wt.display()
        ));
    }

    if log.verbose && !log.quiet {
        log.exec(format!("(cd {}) && git clean -fdx", wt.display()));
    }

    let st = Command::new("git")
        .current_dir(&wt)
        .args(["clean", "-fdx"])
        .stdin(Stdio::null())
        .stdout(if log.verbose && !log.quiet {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .stderr(if log.verbose && !log.quiet {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .status()
        .map_err(|e| format!("failed to run git clean in worktree: {e}"))?;

    if !st.success() {
        return Err(format!("failed to clean worktree at {}", wt.display()));
    }

    Ok(wt)
}

