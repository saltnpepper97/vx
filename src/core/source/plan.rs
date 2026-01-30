// Author Dustin Pilgrim
// License: MIT

use crate::{config::Config, log::Log, managed};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use super::git;
use super::resolve::{resolve_voidpkgs, SrcResolved};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrcUpdate {
    pub name: String,
    pub installed: Option<String>,
    pub candidate: String,
}

/// Used by core/mod.rs for `vx up --all` summary.
pub fn plan_src_updates(
    log: &Log,
    voidpkgs_override: Option<PathBuf>,
    cfg: Option<&Config>,
    pkgs_override: Option<Vec<String>>,
    force: bool,
) -> Result<Vec<SrcUpdate>, String> {
    let resolved = resolve_voidpkgs(voidpkgs_override, cfg)?;

    // NOTE: git::sync_voidpkgs has TTL caching now
    git::sync_voidpkgs(log, &resolved.voidpkgs)?;

    let target = if let Some(pkgs) = pkgs_override {
        pkgs
    } else {
        managed::load_managed()?
    };

    if target.is_empty() {
        return Ok(Vec::new());
    }

    plan_src_updates_with_resolved(log, &resolved, &target, force)
}

pub fn plan_src_updates_with_resolved(
    log: &Log,
    res: &SrcResolved,
    pkgs: &[String],
    force: bool,
) -> Result<Vec<SrcUpdate>, String> {
    // One-shot installed map (big speed win)
    let installed_map = load_installed_pkgver_map().unwrap_or_else(|e| {
        // Non-fatal: fall back to "unknown installed", but warn once.
        log.warn(format!("failed to load installed package list: {e}"));
        HashMap::new()
    });

    let mut out = Vec::new();

    for name in pkgs {
        let tpl = res
            .voidpkgs
            .join("srcpkgs")
            .join(name)
            .join("template");

        let (ver, rev) = match parse_template_version_revision(&tpl) {
            Ok(v) => v,
            Err(e) => {
                log.warn(format!("{name}: {e}"));
                continue;
            }
        };

        let candidate = format!("{name}-{ver}_{rev}");

        // installed is full pkgver (name-version_rev), if installed
        let installed = installed_map.get(name).cloned();

        if !force {
            if let Some(inst) = installed.as_deref() {
                if inst == candidate {
                    continue;
                }
            }
        }

        out.push(SrcUpdate {
            name: name.clone(),
            installed,
            candidate,
        });
    }

    Ok(out)
}

/// Build a HashMap of installed package name -> pkgver (e.g. "firefox" -> "firefox-147.0_1").
/// This avoids N process spawns during planning.
fn load_installed_pkgver_map() -> Result<HashMap<String, String>, String> {
    let out = Command::new("xbps-query")
        .arg("-l")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("failed to run xbps-query -l: {e}"))?;

    if !out.status.success() {
        return Err("xbps-query -l failed".to_string());
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut map: HashMap<String, String> = HashMap::new();

    // Typical lines look like:
    // ii  firefox-147.0_1  Mozilla Firefox Web Browser
    // (status, pkgver, description...)
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut it = line.split_whitespace();
        let status = it.next().unwrap_or("");
        if status != "ii" {
            continue;
        }

        let pkgver = match it.next() {
            Some(v) => v,
            None => continue,
        };

        let name = match pkgname_from_pkgver(pkgver) {
            Some(n) => n,
            None => continue,
        };

        map.insert(name, pkgver.to_string());
    }

    Ok(map)
}

fn pkgname_from_pkgver(pkgver: &str) -> Option<String> {
    let (name, ver) = pkgver.rsplit_once('-')?;
    if ver.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        Some(name.to_string())
    } else {
        None
    }
}

pub fn parse_template_version_revision(path: &Path) -> Result<(String, String), String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read template {}: {e}", path.display()))?;

    let mut version: Option<String> = None;
    let mut revision: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(v) = line.strip_prefix("version=") {
            version = Some(unquote(v.trim()));
        } else if let Some(r) = line.strip_prefix("revision=") {
            revision = Some(unquote(r.trim()));
        }
        if version.is_some() && revision.is_some() {
            break;
        }
    }

    let version = version.ok_or("template missing version=")?;
    let revision = revision.unwrap_or_else(|| "1".to_string());
    Ok((version, revision))
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

