// Author Dustin Pilgrim
// License: MIT

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "vx",
    version,
    about = "Unified Void package front door (xbps + void-packages)",
    long_about = "vx wraps xbps tools and (optionally) void-packages.\n\n\
                  For `vx src ...` you must provide a void-packages path via:\n\
                  - --voidpkgs /path/to/void-packages\n\
                  - VX_VOIDPKGS=/path/to/void-packages\n\
                  - ~/.config/vx/vx.rune (void_packages.path)\n"
)]
pub struct Cli {
    /// Reduce output (errors still print).
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    /// Show executed commands and extra details.
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    /// Override void-packages path for `vx src ...`
    #[arg(long, global = true, value_name = "PATH")]
    pub voidpkgs: Option<PathBuf>,

    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Show VX status (config + void-packages resolution info)
    Status,

    /// Search packages.
    ///
    /// Default searches repos (xbps-query -Rs).
    /// Use -i/--installed to search installed packages (xbps-query -s).
    Search {
        /// Search installed packages instead of repositories.
        #[arg(short = 'i', long)]
        installed: bool,

        /// Search term (one or more words).
        term: Vec<String>,
    },

    /// Show repo package info (xbps-query -R)
    Info {
        /// Package name.
        pkg: String
    },

    /// List installed files for a package (xbps-query -f)
    Files {
        /// Package name.
        pkg: String
    },

    /// Find which installed package owns a path (xbps-query -o)
    Provides {
        /// Path to check (installed file path).
        path: String
    },

    /// Install packages (xbps-install).
    Add {
        /// Assume yes for xbps prompts (-y).
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        /// Packages to install.
        pkgs: Vec<String>,
    },

    /// Remove packages (xbps-remove).
    Rm {
        /// Assume yes for xbps prompts (-y).
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        /// Packages to remove.
        pkgs: Vec<String>,
    },

    /// Update the system and/or tracked source packages.
    ///
    /// - Without --all: updates system via xbps-install -Su.
    /// - With --all: also updates VX-managed source packages via `vx src up --all`.
    Up {
        /// Update system + VX-managed source packages.
        #[arg(short = 'a', long)]
        all: bool,

        /// Show the plan only; do not make changes.
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// For --all, include source packages even if already at candidate version.
        #[arg(short = 'f', long)]
        force: bool,

        /// Skip the single confirmation prompt (implies -y when invoking xbps).
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,
    },

    /// void-packages / xbps-src operations (source builds)
    Src {
        #[command(subcommand)]
        cmd: SrcCmd,
    },
}

#[derive(Subcommand, Debug)]
pub enum SrcCmd {
    /// Build one or more source packages (./xbps-src pkg ...)
    Build {
        pkgs: Vec<String>
    },

    /// Clean build files for one or more source packages (./xbps-src clean ...)
    Clean {
        pkgs: Vec<String>
    },

    /// Lint one or more source packages (./xbps-src lint ...)
    Lint {
        pkgs: Vec<String>
    },

    /// Search void-packages srcpkgs by name.
    ///
    /// Use -i/--installed to only show srcpkgs that are installed on the system.
    Search {
        /// Only show matches that are installed on the system.
        #[arg(short = 'i', long)]
        installed: bool,

        /// Name substring to search for.
        term: String,
    },

    /// Install built packages from the local repo (or rebuild+install).
    ///
    /// Alias: `vx src install ...`
    #[command(alias = "install")]
    Add {
        /// Install even if already installed.
        #[arg(short = 'f', long)]
        force: bool,

        /// Rebuild packages (clean+pkg) before installing.
        #[arg(long)]
        rebuild: bool,

        /// Assume yes for xbps prompts (-y) when installing built packages.
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        pkgs: Vec<String>,
    },

    /// Update source packages (clean+pkg, then install from local repo).
    ///
    /// Use --all to update the VX-managed list.
    Up {
        /// Update the VX-managed source package set.
        #[arg(short = 'a', long)]
        all: bool,

        /// Show the plan only; do not make changes.
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Include packages even if already at candidate version.
        #[arg(short = 'f', long)]
        force: bool,

        /// Skip the single confirmation prompt (implies -y when invoking xbps).
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        /// Packages to update (ignored with --all).
        pkgs: Vec<String>,
    },
}

