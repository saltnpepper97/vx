// Author Dustin Pilgrim
// License: MIT

use crate::{
    config::Config,
    core::xbps::RmOptions,
    log::Log,
    managed,
};
use std::process::{Command, ExitCode, Stdio};
use std::{
    collections::BTreeSet,
    io::{self, IsTerminal, Write},
};

pub fn add(log: &Log, _cfg: Option<&Config>, yes: bool, pkgs: &[String]) -> ExitCode {
    if pkgs.is_empty() {
        log.error("usage: vx add <pkgs...>");
        return ExitCode::from(2);
    }

    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-install");
    if yes {
        cmd.arg("-y");
    }
    cmd.arg("-S");
    cmd.args(pkgs);

    run(log, cmd, "sudo xbps-install ...")
}

pub fn rm(log: &Log, _cfg: Option<&Config>, opts: RmOptions, pkgs: &[String]) -> ExitCode {
    if pkgs.is_empty() && !opts.orphans {
        log.error("usage: vx rm <pkgs...> [--orphans]");
        return ExitCode::from(2);
    }

    // 1) Remove requested packages (if any)
    if !pkgs.is_empty() {
        let mut cmd = Command::new("sudo");
        cmd.arg("xbps-remove");
        apply_xbps_rm_flags(&mut cmd, &opts);
        if opts.recursive {
            cmd.arg("-R");
        }
        cmd.args(&opts.xbps_args);
        cmd.args(pkgs);

        let code = run(log, cmd, "sudo xbps-remove ...");
        if code != ExitCode::SUCCESS {
            return code;
        }

        maybe_untrack_managed(log, opts.yes, pkgs);
    }

    // 2) Optional orphan cleanup pass
    if opts.orphans {
        let mut cmd = Command::new("sudo");
        cmd.arg("xbps-remove");
        apply_xbps_rm_flags(&mut cmd, &opts);
        cmd.args(&opts.xbps_args);
        cmd.arg("-o");

        return run(log, cmd, "sudo xbps-remove -o");
    }

    ExitCode::SUCCESS
}

pub fn up_with_yes(log: &Log, _cfg: Option<&Config>, yes: bool) -> ExitCode {
    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-install");
    if yes {
        cmd.arg("-y");
    }
    cmd.arg("-u");

    run(log, cmd, "sudo xbps-install -u")
}

fn run(log: &Log, mut cmd: Command, label: &str) -> ExitCode {
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    if log.verbose && !log.quiet {
        log.exec(label.to_string());
    }

    match cmd.status() {
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            log.error(format!("failed to run: {e}"));
            ExitCode::from(1)
        }
    }
}

fn apply_xbps_rm_flags(cmd: &mut Command, opts: &RmOptions) {
    if opts.yes {
        cmd.arg("-y");
    }
    if let Some(dir) = &opts.config_dir {
        cmd.arg("-C");
        cmd.arg(dir);
    }
    if let Some(dir) = &opts.cachedir {
        cmd.arg("-c");
        cmd.arg(dir);
    }
    if opts.debug {
        cmd.arg("-d");
    }
    if opts.force_revdeps {
        cmd.arg("-F");
    }
    if opts.force {
        cmd.arg("-f");
    }
    if opts.dry_run {
        cmd.arg("-n");
    }
    for _ in 0..opts.clean_cache {
        cmd.arg("-O");
    }
    if let Some(dir) = &opts.rootdir {
        cmd.arg("-r");
        cmd.arg(dir);
    }
    if opts.xbps_verbose {
        cmd.arg("-v");
    }
}

fn maybe_untrack_managed(log: &Log, yes: bool, pkgs: &[String]) {
    let managed = match managed::load_managed() {
        Ok(v) => v,
        Err(e) => {
            log.warn(format!("failed to load managed src list: {e}"));
            return;
        }
    };

    if managed.is_empty() {
        return;
    }

    let tracked: BTreeSet<&str> = managed.iter().map(String::as_str).collect();
    let mut to_untrack: Vec<String> = Vec::new();
    for p in pkgs {
        if tracked.contains(p.as_str()) {
            to_untrack.push(p.clone());
        }
    }

    if to_untrack.is_empty() {
        return;
    }

    let should_untrack = if yes {
        true
    } else if io::stdin().is_terminal() && io::stdout().is_terminal() {
        println!("tracked source packages being removed:");
        for p in &to_untrack {
            println!("  {p}");
        }
        confirm_yes_default("Also remove them from the vx source list?")
    } else {
        true
    };

    if !should_untrack {
        return;
    }

    if let Err(e) = managed::remove_managed(&to_untrack) {
        log.warn(format!(
            "removed packages but failed to update managed list: {e}"
        ));
    } else if log.verbose && !log.quiet {
        log.exec(format!("untracked: {}", to_untrack.join(", ")));
    }
}

fn confirm_yes_default(prompt: &str) -> bool {
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
