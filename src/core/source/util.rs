// Author Dustin Pilgrim
// License: MIT

use crate::core::xbps::SysUpdate;
use crate::log::Log;
use std::io::{self, Write};

use super::plan::SrcUpdate;

pub fn print_up_all_summary(log: &Log, sys: &[SysUpdate], src: &[SrcUpdate]) {
    if log.quiet {
        return;
    }

    println!("Summary:");
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

pub fn print_src_plan_summary(log: &Log, plan: &[SrcUpdate]) {
    if log.quiet {
        return;
    }
    println!("vx: source update plan");
    for p in plan {
        let from = p.installed.as_deref().unwrap_or("<not installed>");
        println!("  {}  {} → {}", p.name, from, p.candidate);
    }
}

