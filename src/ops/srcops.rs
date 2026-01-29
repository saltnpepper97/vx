// Author Dustin Pilgrim
// License: MIT

use crate::{cli::SrcCmd, config::Config, log::Log, managed, ops::xbps::SysUpdate};
use std::{
    env,
    ffi::OsString,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
};

#[derive(Debug, Clone)]
pub struct SrcUpdate {
    pub name: String,
    pub installed: Option<String>,
    pub candidate: String,
}

pub fn dispatch_src(
    log: &Log,
    voidpkgs_override: Option<PathBuf>,
    cfg: Option<&Config>,
    cmd: SrcCmd,
) -> ExitCode {
    if let SrcCmd::Add { force, rebuild, .. } = &cmd {
        if *force && *rebuild {
            log.error("use either --force or --rebuild, not both");
            return ExitCode::from(2);
        }
    }

    let resolved = match resolve_voidpkgs(voidpkgs_override, cfg) {
        Ok(r) => r,
        Err(e) => {
            log.error(e);
            return ExitCode::from(2);
        }
    };

    match cmd {
        SrcCmd::Search { installed, term } => src_search(log, &resolved, installed, &term),

        SrcCmd::Build { pkgs } => build(log, &resolved, &pkgs),
        SrcCmd::Clean { pkgs } => clean(log, &resolved, &pkgs),
        SrcCmd::Lint { pkgs } => lint(log, &resolved, &pkgs),

        SrcCmd::Add {
            force,
            rebuild,
            yes,
            pkgs,
        } => {
            let code = if rebuild {
                src_up(log, &resolved, yes, &pkgs)
            } else {
                add_from_local_repo(log, &resolved, force, yes, &pkgs)
            };

            if code == ExitCode::SUCCESS {
                if let Err(e) = managed::add_managed(&pkgs.to_vec()) {
                    log.warn(format!("failed to update managed-src list: {e}"));
                }
            }
            code
        }

        SrcCmd::Up {
            all,
            dry_run,
            force,
            yes,
            pkgs,
        } => {
            let target = if all {
                match managed::load_managed() {
                    Ok(v) => v,
                    Err(e) => {
                        log.error(e);
                        return ExitCode::from(2);
                    }
                }
            } else {
                pkgs
            };

            if target.is_empty() {
                log.info("no source packages specified.");
                if all {
                    log.info("hint: install one with `vx src add <pkg>`");
                } else {
                    log.info("hint: use `vx src up <pkg...>` or `vx src up --all`");
                }
                return ExitCode::SUCCESS;
            }

            let plan = match plan_src_updates_with_resolved(log, &resolved, &target, force) {
                Ok(v) => v,
                Err(e) => {
                    log.error(e);
                    return ExitCode::from(1);
                }
            };

            if plan.is_empty() {
                log.info("all source packages are already up to date.");
                return ExitCode::SUCCESS;
            }

            print_src_plan_summary(log, &plan);

            if dry_run {
                return ExitCode::SUCCESS;
            }

            if !yes {
                if !confirm_once("Proceed?") {
                    log.info("aborted.");
                    return ExitCode::SUCCESS;
                }
            }

            let pkgs_to_update: Vec<String> = plan.iter().map(|p| p.name.clone()).collect();
            src_up(log, &resolved, yes, &pkgs_to_update)
        }
    }
}

fn src_search(log: &Log, res: &SrcResolved, installed_only: bool, term: &str) -> ExitCode {
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

/// Used by ops/mod.rs for `vx up --all` summary.
pub fn plan_src_updates(
    log: &Log,
    voidpkgs_override: Option<PathBuf>,
    cfg: Option<&Config>,
    pkgs_override: Option<Vec<String>>,
    force: bool,
) -> Result<Vec<SrcUpdate>, String> {
    let resolved = resolve_voidpkgs(voidpkgs_override, cfg)?;
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

pub fn print_up_all_summary(log: &Log, sys: &[SysUpdate], src: &[SrcUpdate]) {
    if log.quiet {
        return;
    }

    println!("vx: update --all summary");

    println!("  system: xbps-install -Su");
    if sys.is_empty() {
        println!("    (no system updates found)");
    } else {
        for u in sys {
            println!("    {}  {} → {}", u.name, u.from, u.to);
        }
    }

    println!("  source: vx-managed packages");
    if src.is_empty() {
        println!("    (no source updates found)");
    } else {
        for p in src {
            let from = p.installed.as_deref().unwrap_or("<not installed>");
            println!("    {}  {} → {}", p.name, from, p.candidate);
        }
    }
}

pub fn confirm_once(prompt: &str) -> bool {
    print!("{prompt} [Y/n] ");
    let _ = io::stdout().flush();
    let mut s = String::new();
    if io::stdin().read_line(&mut s).is_ok() {
        let t = s.trim().to_lowercase();
        t.is_empty() || matches!(t.as_str(), "y" | "yes")
    } else {
        false
    }
}

#[derive(Debug, Clone)]
struct SrcResolved {
    voidpkgs: PathBuf,
    local_repo_rel: PathBuf,
    use_nonfree: bool,
}

fn resolve_voidpkgs(
    voidpkgs_override: Option<PathBuf>,
    cfg: Option<&Config>,
) -> Result<SrcResolved, String> {
    let mut local_repo_rel = PathBuf::from("hostdir/binpkgs");
    let mut use_nonfree = true;

    if let Some(c) = cfg {
        if !c.local_repo_rel.as_os_str().is_empty() {
            local_repo_rel = c.local_repo_rel.clone();
        }
        use_nonfree = c.use_nonfree;
    }

    // 1) CLI override
    if let Some(p) = voidpkgs_override {
        return Ok(SrcResolved {
            voidpkgs: p,
            local_repo_rel,
            use_nonfree,
        });
    }

    // 2) env var
    if let Ok(v) = env::var("VX_VOIDPKGS") {
        let p = PathBuf::from(v);
        if !p.as_os_str().is_empty() {
            return Ok(SrcResolved {
                voidpkgs: p,
                local_repo_rel,
                use_nonfree,
            });
        }
    }

    // 3) config (now Option<PathBuf>)
    if let Some(c) = cfg {
        if let Some(p) = &c.void_packages_path {
            if !p.as_os_str().is_empty() {
                return Ok(SrcResolved {
                    voidpkgs: p.clone(),
                    local_repo_rel,
                    use_nonfree,
                });
            }
        }
    }

    Err(
        "vx src requires a void-packages path.\n\
         Provide one of:\n\
         - --voidpkgs /path/to/void-packages\n\
         - VX_VOIDPKGS=/path/to/void-packages\n\
         - ~/.config/vx/vx.rune with void_packages.path\n"
            .to_string(),
    )
}

fn build(log: &Log, res: &SrcResolved, pkgs: &[String]) -> ExitCode {
    if let Err(code) = need_pkgs(log, "vx src build", pkgs) {
        return code;
    }
    run_xbps_src(log, &res.voidpkgs, join_args("pkg", pkgs))
}

fn clean(log: &Log, res: &SrcResolved, pkgs: &[String]) -> ExitCode {
    if let Err(code) = need_pkgs(log, "vx src clean", pkgs) {
        return code;
    }
    run_xbps_src(log, &res.voidpkgs, join_args("clean", pkgs))
}

fn lint(log: &Log, res: &SrcResolved, pkgs: &[String]) -> ExitCode {
    if let Err(code) = need_pkgs(log, "vx src lint", pkgs) {
        return code;
    }
    run_xbps_src(log, &res.voidpkgs, join_args("lint", pkgs))
}

fn src_up(log: &Log, res: &SrcResolved, yes: bool, pkgs: &[String]) -> ExitCode {
    let c = run_xbps_src(log, &res.voidpkgs, join_args("clean", pkgs));
    if c != ExitCode::SUCCESS {
        return c;
    }

    let c = run_xbps_src(log, &res.voidpkgs, join_args("pkg", pkgs));
    if c != ExitCode::SUCCESS {
        return c;
    }

    let c = add_from_local_repo(log, res, true, yes, pkgs);

    if c == ExitCode::SUCCESS {
        if let Err(e) = managed::add_managed(&pkgs.to_vec()) {
            log.warn(format!("failed to update managed-src list: {e}"));
        }
    }

    c
}

fn need_pkgs(log: &Log, usage: &str, pkgs: &[String]) -> Result<(), ExitCode> {
    if pkgs.is_empty() {
        log.error(format!("usage: {usage} <pkg> [pkg...]"));
        Err(ExitCode::from(2))
    } else {
        Ok(())
    }
}

fn join_args(sub: &str, pkgs: &[String]) -> Vec<OsString> {
    let mut out = Vec::with_capacity(1 + pkgs.len());
    out.push(OsString::from(sub));
    out.extend(pkgs.iter().cloned().map(OsString::from));
    out
}

fn run_xbps_src(log: &Log, voidpkgs: &Path, args: Vec<OsString>) -> ExitCode {
    if !voidpkgs.join("xbps-src").is_file() {
        log.error(format!(
            "not a void-packages directory (missing ./xbps-src): {}",
            voidpkgs.display()
        ));
        return ExitCode::from(2);
    }

    if log.verbose && !log.quiet {
        let mut s = String::from("./xbps-src");
        for a in &args {
            s.push(' ');
            s.push_str(&a.to_string_lossy());
        }
        log.exec(format!("(cd {}) && {}", voidpkgs.display(), s));
    }

    match Command::new("./xbps-src")
        .current_dir(voidpkgs)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
    {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(e) => {
            log.error(format!("failed to run ./xbps-src: {e}"));
            ExitCode::from(1)
        }
    }
}

fn add_from_local_repo(
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

    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-install");
    cmd.arg("-R").arg(&base);

    let nonfree = base.join("nonfree");
    if res.use_nonfree && nonfree.is_dir() {
        cmd.arg("-R").arg(&nonfree);
    }

    if force {
        cmd.arg("-f");
    }
    if yes {
        cmd.arg("-y");
    }

    cmd.args(&to_install);

    if log.verbose && !log.quiet {
        let mut s = format!("sudo xbps-install -R {}", base.display());
        if res.use_nonfree && nonfree.is_dir() {
            s.push_str(&format!(" -R {}", nonfree.display()));
        }
        if force {
            s.push_str(" -f");
        }
        if yes {
            s.push_str(" -y");
        }
        for p in &to_install {
            s.push(' ');
            s.push_str(p);
        }
        log.exec(s);
    }

    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    match cmd.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(e) => {
            log.error(format!("failed to run sudo xbps-install: {e}"));
            ExitCode::from(1)
        }
    }
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

fn plan_src_updates_with_resolved(
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

fn parse_template_version_revision(path: &Path) -> Result<(String, String), String> {
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

fn print_src_plan_summary(log: &Log, plan: &[SrcUpdate]) {
    if log.quiet {
        return;
    }
    println!("vx: source update plan");
    for p in plan {
        let from = p.installed.as_deref().unwrap_or("<not installed>");
        println!("  {}  {} → {}", p.name, from, p.candidate);
    }
}

