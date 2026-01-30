// Author Dustin Pilgrim
// License: MIT

use crate::{config::Config, log::Log, managed};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
        let installed = installed_pkgver(name)?;

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

fn installed_pkgver(pkg: &str) -> Result<Option<String>, String> {
    let out = Command::new("xbps-query")
        .arg("-p")
        .arg("pkgver")
        .arg(pkg)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("failed to run xbps-query: {e}"))?;

    if !out.status.success() {
        return Ok(None);
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s))
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

