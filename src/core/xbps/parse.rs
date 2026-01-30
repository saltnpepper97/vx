// Author Dustin Pilgrim
// License: MIT

use super::plan::SysUpdate;

/// Parse `xbps-install -Sun` (or `-un`) output.
///
/// Supports:
///  A) table format:
///     Name Action Version New version Download size
///     firefox update 147.0_1 147.0.2_1 82MB
///
///  B) column format:
///     <pkgver> <action> <arch> <repo> ...
pub fn parse_xbps_sun_plan<F>(text: &str, installed_pkgver: F) -> Result<Vec<SysUpdate>, String>
where
    F: Fn(&str) -> Result<Option<String>, String>,
{
    let mut out: Vec<SysUpdate> = Vec::new();

    let mut in_table = false;
    let mut saw_table_row = false;

    for raw in text.lines() {
        let line = raw.trim();

        if line.is_empty() {
            if in_table && !saw_table_row {
                continue;
            }
            in_table = false;
            saw_table_row = false;
            continue;
        }

        if line.starts_with("[*]")
            || line.starts_with("=>")
            || line.starts_with("xbps-install:")
            || line.starts_with("Size to download:")
            || line.starts_with("Size required on disk:")
            || line.starts_with("Space available on disk:")
            || line.starts_with("Do you want to continue?")
            || line.starts_with("Aborting!")
        {
            continue;
        }

        if line.starts_with("Name")
            && line.contains("Action")
            && (line.contains("Version") || line.contains("Current"))
            && (line.contains("New") || line.contains("New version"))
        {
            in_table = true;
            saw_table_row = false;
            continue;
        }

        // A) table rows
        if in_table {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 4 {
                continue;
            }

            let name = cols[0].to_string();
            let action = cols[1];

            if !matches!(action, "update" | "install" | "reinstall" | "downgrade") {
                continue;
            }

            let oldver = cols[2];
            let newver = cols[3];

            let from = format!("{name}-{oldver}");
            let to = format!("{name}-{newver}");

            out.push(SysUpdate { name, from, to });
            saw_table_row = true;
            continue;
        }

        // B) column-ish rows
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 2 {
            continue;
        }

        let pkgver = cols[0];
        let action = cols[1];

        if !matches!(action, "update" | "install" | "reinstall" | "downgrade") {
            continue;
        }

        let name = match pkgname_from_pkgver(pkgver) {
            Some(n) => n,
            None => continue,
        };

        let from = match installed_pkgver(&name)? {
            Some(v) => v,
            None => "<not installed>".to_string(),
        };

        out.push(SysUpdate {
            name,
            from,
            to: pkgver.to_string(),
        });
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name == b.name);

    Ok(out)
}

fn pkgname_from_pkgver(pkgver: &str) -> Option<String> {
    let (name, ver) = pkgver.rsplit_once('-')?;
    if ver.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        Some(name.to_string())
    } else {
        None
    }
}

pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' {
            if it.peek() == Some(&'[') {
                it.next(); // '['
                while let Some(n) = it.next() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
                continue;
            }
        }
        out.push(c);
    }
    out
}

