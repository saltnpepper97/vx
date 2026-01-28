// Author Dustin Pilgrim
// License: MIT

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name="vx", version, about="Unified Void package front door (xbps + void-packages)")]
pub struct Cli {
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    #[arg(long, global = true)]
    pub voidpkgs: Option<PathBuf>,

    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    Status,

    /// Search packages in repositories (xbps-query -Rs)
    Search {
        /// Search installed packages instead of repos (xbps-query -s)
        #[arg(short = 'i', long)]
        installed: bool,

        term: Vec<String>,
    },

    /// Show repo package info (xbps-query -R)
    Info { pkg: String },

    /// List installed files for a package (xbps-query -f)
    Files { pkg: String },

    /// Find which installed package owns a path (xbps-query -o)
    Provides { path: String },

    Add {
        /// Assume yes for xbps prompts (-y)
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        pkgs: Vec<String>,
    },

    Rm {
        /// Assume yes for xbps prompts (-y)
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        pkgs: Vec<String>,
    },

    Up {
        #[arg(short = 'a', long)]
        all: bool,

        #[arg(short = 'n', long)]
        dry_run: bool,

        #[arg(short = 'f', long)]
        force: bool,

        /// Skip the single confirmation prompt (implies -y for xbps)
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,
    },

    Src {
        #[command(subcommand)]
        cmd: SrcCmd,
    },
}

#[derive(Subcommand, Debug)]
pub enum SrcCmd {
    Build { pkgs: Vec<String> },

    Clean { pkgs: Vec<String> },

    Lint { pkgs: Vec<String> },

    /// Search void-packages srcpkgs by name (optionally only those installed)
    Search {
        /// Only show matches that are installed on the system (xbps-query)
        #[arg(short = 'i', long)]
        installed: bool,

        term: String,
    },

    #[command(alias = "install")]
    Add {
        #[arg(short = 'f', long)]
        force: bool,

        #[arg(long)]
        rebuild: bool,

        /// Assume yes for xbps prompts (-y) when installing built packages
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        pkgs: Vec<String>,
    },

    Up {
        #[arg(short = 'a', long)]
        all: bool,

        #[arg(short = 'n', long)]
        dry_run: bool,

        #[arg(short = 'f', long)]
        force: bool,

        /// Skip the single confirmation prompt (implies -y for xbps)
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        pkgs: Vec<String>,
    },
}

