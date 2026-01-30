// Author Dustin Pilgrim
// License: MIT

use crate::{cli::Cli, config::Config, managed, paths::user_config_path};
use std::{env, path::PathBuf, process::ExitCode};

pub fn run_status(_log: &crate::log::Log, cli: &Cli, cfg: Option<&Config>) -> ExitCode {
    println!("version: {}", env!("CARGO_PKG_VERSION"));

    match user_config_path() {
        Ok(p) => {
            if p.exists() {
                println!("config: loaded ({})", p.display());
            } else {
                println!("config: none (expected at {})", p.display());
            }
        }
        Err(e) => {
            eprintln!("error: failed to resolve config path: {e}");
            return ExitCode::from(2);
        }
    }

    if let Some(c) = cfg {
        println!("debug: {}", c.debug);
    } else {
        println!("debug: false");
    }

    let (voidpkgs, source) = resolve_voidpkgs_for_status(cli, cfg);
    match voidpkgs {
        Some(p) => println!("voidpkgs: {} ({})", p.display(), source),
        None => println!("voidpkgs: unset (needed for `vx src ...`)"),
    }

    if let Some(c) = cfg {
        println!(
            "src repo: {} (use_nonfree={})",
            c.local_repo_rel.display(),
            c.use_nonfree
        );
    } else {
        println!("src repo: hostdir/binpkgs (use_nonfree=true)");
    }

    match managed::load_managed() {
        Ok(list) => {
            println!("managed: {} package(s)", list.len());
            if !list.is_empty() {
                let show = 10usize;
                let head = list.iter().take(show).cloned().collect::<Vec<_>>();
                println!("managed list: {}", head.join(" "));
                if list.len() > show {
                    println!("managed list: (+{} more)", list.len() - show);
                }
            }
        }
        Err(e) => {
            println!("managed: unavailable ({e})");
        }
    }

    println!("flags: quiet={} verbose={}", cli.quiet, cli.verbose);

    ExitCode::SUCCESS
}

fn resolve_voidpkgs_for_status(cli: &Cli, cfg: Option<&Config>) -> (Option<PathBuf>, &'static str) {
    if let Some(p) = &cli.voidpkgs {
        if !p.as_os_str().is_empty() {
            return (Some(p.clone()), "cli");
        }
    }

    if let Ok(v) = env::var("VX_VOIDPKGS") {
        let p = PathBuf::from(v);
        if !p.as_os_str().is_empty() {
            return (Some(p), "env");
        }
    }

    if let Some(c) = cfg {
        if let Some(p) = &c.void_packages_path {
            if !p.as_os_str().is_empty() {
                return (Some(p.clone()), "config");
            }
        }
    }

    (None, "unset")
}

