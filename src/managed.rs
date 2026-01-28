// Author Dustin Pilgrim
// License: MIT

use crate::paths::managed_src_path;
use rune_cfg::RuneConfig;
use std::{
    collections::BTreeSet,
    fs,
    io,
    path::Path,
};

pub fn load_managed() -> Result<Vec<String>, String> {
    let path = managed_src_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let cfg = RuneConfig::from_file(path.to_str().ok_or("invalid managed-src path")?)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;

    // Expect: packages ["a" "b" ...]
    let pkgs: Vec<String> = cfg
        .get("packages")
        .unwrap_or_else(|_| Vec::new());

    Ok(dedupe_sorted(pkgs))
}

pub fn add_managed(pkgs: &[String]) -> Result<(), String> {
    let path = managed_src_path()?;
    let mut existing = if path.exists() { load_managed()? } else { Vec::new() };

    existing.extend(pkgs.iter().cloned());
    let merged = dedupe_sorted(existing);

    write_manifest(&path, &merged).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

fn dedupe_sorted(mut pkgs: Vec<String>) -> Vec<String> {
    let mut set = BTreeSet::new();
    for p in pkgs.drain(..) {
        let t = p.trim();
        if !t.is_empty() {
            set.insert(t.to_string());
        }
    }
    set.into_iter().collect()
}

fn write_manifest(path: &Path, pkgs: &[String]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    let mut out = String::new();
    out.push_str("@author \"vx\"\n");
    out.push_str("@description \"Source packages managed by vx\"\n\n");
    out.push_str("packages [\n");
    for p in pkgs {
        out.push_str("  \"");
        out.push_str(&escape_string(p));
        out.push_str("\"\n");
    }
    out.push_str("]\n");

    fs::write(path, out)
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

