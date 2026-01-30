// Author Dustin Pilgrim
// License: MIT

use crate::log::Log;
use std::process::{Command, ExitCode, Stdio};

use super::resolve::SrcResolved;
use super::plan::parse_template_version_revision;

pub fn src_search(log: &Log, res: &SrcResolved, installed_only: bool, term: &str) -> ExitCode {
    let needle = term.trim();
    if needle.is_empty() {
        log.error("usage: vx src search <term>");
        return ExitCode::from(2);
    }

    let srcpkgs = res.voidpkgs.join("srcpkgs");
    if !srcpkgs.is_dir() {
        log.error(format!(
            "void-packages missing ./srcpkgs: {}",
            srcpkgs.display()
        ));
        return ExitCode::from(2);
    }

    let needle_lc = needle.to_ascii_lowercase();
    let mut hits: Vec<(String, Option<String>, bool)> = Vec::new();

    let rd = match std::fs::read_dir(&srcpkgs) {
        Ok(v) => v,
        Err(e) => {
            log.error(format!("failed to read {}: {e}", srcpkgs.display()));
            return ExitCode::from(1);
        }
    };

    for ent in rd.flatten() {
        let ft = match ent.file_type() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if !ft.is_dir() {
            continue;
        }

        let name = ent.file_name().to_string_lossy().to_string();
        if !name.to_ascii_lowercase().contains(&needle_lc) {
            continue;
        }

        let installed = match is_installed_system(&name) {
            Ok(v) => v,
            Err(_) => false,
        };

        if installed_only && !installed {
            continue;
        }

        let tpl = ent.path().join("template");
        let ver = match parse_template_version_revision(&tpl) {
            Ok((v, r)) => Some(format!("{v}_{r}")),
            Err(_) => None,
        };

        hits.push((name, ver, installed));
    }

    hits.sort_by(|a, b| a.0.cmp(&b.0));

    if hits.is_empty() {
        log.info("no matches.");
        return ExitCode::SUCCESS;
    }

    for (name, ver, installed) in hits {
        let mark = if installed { "[*]" } else { "[-]" };
        if let Some(v) = ver {
            println!("{mark} {:<20} {}", name, v);
        } else {
            println!("{mark} {name}");
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

