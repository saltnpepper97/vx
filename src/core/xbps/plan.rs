// Author Dustin Pilgrim
// License: MIT

use crate::{config::Config, log::Log};
use std::process::{Command, Stdio};

use super::{parse, query};

#[derive(Debug, Clone)]
pub struct SysUpdate {
    pub name: String,
    pub from: String,
    pub to: String,
}

/// Dry-run system update and parse versions.
///
/// Key behavior change:
/// - Always sync repositories first (`sudo xbps-install -S`)
/// - Then run dry-run update plan (`sudo xbps-install -un`)
pub fn plan_system_updates(log: &Log, _cfg: Option<&Config>) -> Result<Vec<SysUpdate>, String> {
    // 1) Sync repodata first
    {
        let mut sync = Command::new("sudo");
        sync.arg("xbps-install");
        sync.args(["-S"]);
        sync.env("XBPS_COLORS", "0");
        sync.stdin(Stdio::inherit());
        sync.stdout(Stdio::piped());
        sync.stderr(Stdio::piped());

        if log.verbose && !log.quiet {
            log.exec("sudo xbps-install -S".to_string());
        }

        let out = sync
            .output()
            .map_err(|e| format!("failed to run xbps-install -S: {e}"))?;

        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if err.is_empty() {
                return Err(format!(
                    "xbps-install -S failed (exit={})",
                    out.status.code().unwrap_or(1)
                ));
            }
            return Err(format!("xbps-install -S failed: {err}"));
        }
    }

    // 2) Dry-run update plan based on freshly synced repodata
    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-install");
    cmd.args(["-un"]);
    cmd.env("XBPS_COLORS", "0");
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    if log.verbose && !log.quiet {
        log.exec("sudo xbps-install -un".to_string());
    }

    let out = cmd
        .output()
        .map_err(|e| format!("failed to run xbps-install -un: {e}"))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.is_empty() {
            return Err(format!(
                "xbps-install -un failed (exit={})",
                out.status.code().unwrap_or(1)
            ));
        }
        return Err(format!("xbps-install -un failed: {err}"));
    }

    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let text = parse::strip_ansi(&text);

    let plan = parse::parse_xbps_sun_plan(&text, |name| query::installed_pkgver(name))?;

    if plan.is_empty()
        && (text.contains("Name")
            && text.contains("Action")
            && (text.contains("Version") || text.contains("Current"))
            && (text.contains("New") || text.contains("New version")))
    {
        return Err(
            "failed to parse xbps dry-run output (format changed); refusing to report empty plan"
                .to_string(),
        );
    }

    Ok(plan)
}

