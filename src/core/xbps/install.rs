// Author Dustin Pilgrim
// License: MIT

use crate::{
    config::Config,
    core::xbps::{AddOptions, RmOptions},
    log::Log,
    managed,
};
use std::{
    ffi::OsString,
    collections::BTreeSet,
    io::{self, IsTerminal, Write},
    process::{Command, ExitCode, Stdio},
};

pub fn add(log: &Log, _cfg: Option<&Config>, opts: AddOptions, pkgs: &[String]) -> ExitCode {
    if pkgs.is_empty() {
        log.error("usage: vx add <pkgs...>");
        return ExitCode::from(2);
    }

    let mut cmd = Command::new("sudo");
    cmd.arg("xbps-install");
    cmd.args(xbps_install_args(&opts, pkgs));

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
        cmd.args(xbps_remove_args(&opts, pkgs));

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
        cmd.args(xbps_remove_orphan_args(&opts));

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

fn xbps_install_args(opts: &AddOptions, pkgs: &[String]) -> Vec<OsString> {
    let mut out = Vec::new();

    if opts.yes {
        out.push("-y".into());
    }
    if opts.automatic {
        out.push("-A".into());
    }
    if let Some(dir) = &opts.config_dir {
        out.push("-C".into());
        out.push(dir.as_os_str().to_os_string());
    }
    if let Some(dir) = &opts.cachedir {
        out.push("-c".into());
        out.push(dir.as_os_str().to_os_string());
    }
    if opts.debug {
        out.push("-d".into());
    }
    if opts.download_only {
        out.push("-D".into());
    }
    for _ in 0..opts.force {
        out.push("-f".into());
    }
    if opts.ignore_conf_repos {
        out.push("-i".into());
    }
    if opts.ignore_file_conflicts {
        out.push("-I".into());
    }
    if opts.unpack_only {
        out.push("-U".into());
    }
    if opts.memory_sync {
        out.push("-M".into());
    }
    if opts.dry_run {
        out.push("-n".into());
    }
    for repo in &opts.repositories {
        out.push("-R".into());
        out.push(repo.into());
    }
    if let Some(dir) = &opts.rootdir {
        out.push("-r".into());
        out.push(dir.as_os_str().to_os_string());
    }
    if opts.reproducible {
        out.push("--reproducible".into());
    }
    if opts.staging {
        out.push("--staging".into());
    }
    if opts.sync {
        out.push("-S".into());
    }
    if opts.update {
        out.push("-u".into());
    }
    if opts.xbps_verbose {
        out.push("-v".into());
    }

    out.extend(opts.xbps_args.iter().cloned().map(OsString::from));
    out.extend(pkgs.iter().cloned().map(OsString::from));
    out
}

fn xbps_rm_common_args(opts: &RmOptions) -> Vec<OsString> {
    let mut out = Vec::new();

    if opts.yes {
        out.push("-y".into());
    }
    if let Some(dir) = &opts.config_dir {
        out.push("-C".into());
        out.push(dir.as_os_str().to_os_string());
    }
    if let Some(dir) = &opts.cachedir {
        out.push("-c".into());
        out.push(dir.as_os_str().to_os_string());
    }
    if opts.debug {
        out.push("-d".into());
    }
    if opts.force_revdeps {
        out.push("-F".into());
    }
    if opts.force {
        out.push("-f".into());
    }
    if opts.dry_run {
        out.push("-n".into());
    }
    for _ in 0..opts.clean_cache {
        out.push("-O".into());
    }
    if let Some(dir) = &opts.rootdir {
        out.push("-r".into());
        out.push(dir.as_os_str().to_os_string());
    }
    if opts.xbps_verbose {
        out.push("-v".into());
    }

    out
}

fn xbps_remove_args(opts: &RmOptions, pkgs: &[String]) -> Vec<OsString> {
    let mut out = xbps_rm_common_args(opts);
    if opts.recursive {
        out.push("-R".into());
    }
    out.extend(opts.xbps_args.iter().cloned().map(OsString::from));
    out.extend(pkgs.iter().cloned().map(OsString::from));
    out
}

fn xbps_remove_orphan_args(opts: &RmOptions) -> Vec<OsString> {
    let mut out = xbps_rm_common_args(opts);
    out.extend(opts.xbps_args.iter().cloned().map(OsString::from));
    out.push("-o".into());
    out
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

#[cfg(test)]
mod tests {
    use super::{xbps_install_args, xbps_remove_args, xbps_remove_orphan_args};
    use crate::core::xbps::{AddOptions, RmOptions};
    use std::{ffi::OsString, path::PathBuf};

    fn s(args: Vec<OsString>) -> Vec<String> {
        args.into_iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect()
    }

    fn add_opts() -> AddOptions {
        AddOptions {
            yes: false,
            automatic: false,
            config_dir: None,
            cachedir: None,
            debug: false,
            download_only: false,
            force: 0,
            ignore_conf_repos: false,
            ignore_file_conflicts: false,
            unpack_only: false,
            memory_sync: false,
            dry_run: false,
            repositories: Vec::new(),
            rootdir: None,
            reproducible: false,
            staging: false,
            sync: true,
            update: false,
            xbps_verbose: false,
            xbps_args: Vec::new(),
        }
    }

    fn rm_opts() -> RmOptions {
        RmOptions {
            yes: false,
            config_dir: None,
            cachedir: None,
            debug: false,
            force_revdeps: false,
            force: false,
            dry_run: false,
            clean_cache: 0,
            orphans: false,
            recursive: true,
            rootdir: None,
            xbps_verbose: false,
            xbps_args: Vec::new(),
        }
    }

    #[test]
    fn install_args_keep_sync_default_and_append_pkgs() {
        let opts = add_opts();
        let pkgs = vec!["ripgrep".to_string(), "fd".to_string()];
        assert_eq!(s(xbps_install_args(&opts, &pkgs)), vec!["-S", "ripgrep", "fd"]);
    }

    #[test]
    fn install_args_include_selected_flags_and_passthrough_before_pkgs() {
        let mut opts = add_opts();
        opts.yes = true;
        opts.automatic = true;
        opts.config_dir = Some(PathBuf::from("/etc/xbps.d"));
        opts.force = 2;
        opts.repositories = vec!["https://repo-1".to_string(), "https://repo-2".to_string()];
        opts.sync = false;
        opts.update = true;
        opts.xbps_verbose = true;
        opts.xbps_args = vec!["--staging".to_string(), "--foo".to_string()];

        let out = s(xbps_install_args(&opts, &["hello".to_string()]));
        assert_eq!(
            out,
            vec![
                "-y",
                "-A",
                "-C",
                "/etc/xbps.d",
                "-f",
                "-f",
                "-R",
                "https://repo-1",
                "-R",
                "https://repo-2",
                "-u",
                "-v",
                "--staging",
                "--foo",
                "hello",
            ]
        );
    }

    #[test]
    fn remove_args_default_recursive_and_pkg_order() {
        let opts = rm_opts();
        let out = s(xbps_remove_args(&opts, &["ripgrep".to_string()]));
        assert_eq!(out, vec!["-R", "ripgrep"]);
    }

    #[test]
    fn remove_args_no_recursive_with_flags_and_passthrough() {
        let mut opts = rm_opts();
        opts.recursive = false;
        opts.yes = true;
        opts.clean_cache = 2;
        opts.rootdir = Some(PathBuf::from("/mnt/root"));
        opts.xbps_args = vec!["--foo".to_string()];
        let out = s(xbps_remove_args(&opts, &["a".to_string(), "b".to_string()]));
        assert_eq!(
            out,
            vec!["-y", "-O", "-O", "-r", "/mnt/root", "--foo", "a", "b"]
        );
    }

    #[test]
    fn remove_orphan_args_forward_common_and_add_o() {
        let mut opts = rm_opts();
        opts.debug = true;
        opts.xbps_args = vec!["--foo".to_string()];
        let out = s(xbps_remove_orphan_args(&opts));
        assert_eq!(out, vec!["-d", "--foo", "-o"]);
    }
}
