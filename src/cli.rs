// Author Dustin Pilgrim
// License: MIT

use clap::{ArgAction, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "vx",
    version,
    about = "Void Linux package manager front-end",
    long_about = "vx wraps xbps and xbps-src into a single intuitive tool.\n\n\
                  Think pacman/apt feel for daily Void Linux usage.\n\n\
                  For `vx src` commands, provide a void-packages path via:\n\
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

    /// Override void-packages path.
    #[arg(long, global = true, value_name = "PATH")]
    pub voidpkgs: Option<PathBuf>,

    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Show vx status (config + void-packages info).
    Status,

    /// Search available packages (xbps-query -Rs).
    Search {
        /// Search term.
        term: Vec<String>,
    },

    /// Show package information (xbps-query -R).
    Info {
        /// Package name.
        pkg: String,
    },

    /// List installed files for a package (xbps-query -f).
    Files {
        /// Package name.
        pkg: String,
    },

    /// List installed packages (xbps-query -l).
    List {
        /// Filter by name substring.
        term: Option<String>,
    },

    /// Find which package owns a path (xbps-query -o).
    Owns {
        /// Path to check.
        path: String,
    },

    /// Install packages from repositories (xbps-install).
    Add {
        /// Assume yes.
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        /// Automatic installation mode.
        #[arg(short = 'A', long = "automatic")]
        automatic: bool,

        /// Path to xbps confdir.
        #[arg(short = 'C', long = "config", value_name = "DIR")]
        config_dir: Option<PathBuf>,

        /// Path to xbps package cache.
        #[arg(short = 'c', long, value_name = "DIR")]
        cachedir: Option<PathBuf>,

        /// Enable xbps debug output.
        #[arg(short = 'd', long)]
        debug: bool,

        /// Download packages only.
        #[arg(short = 'D', long = "download-only")]
        download_only: bool,

        /// Force reinstallation (repeat for stronger force).
        #[arg(short = 'f', long, action = ArgAction::Count)]
        force: u8,

        /// Ignore repositories defined in xbps.d.
        #[arg(short = 'i', long = "ignore-conf-repos")]
        ignore_conf_repos: bool,

        /// Ignore detected file conflicts.
        #[arg(short = 'I', long = "ignore-file-conflicts")]
        ignore_file_conflicts: bool,

        /// Unpack only; do not configure.
        #[arg(short = 'U', long = "unpack-only")]
        unpack_only: bool,

        /// Keep repository metadata in memory.
        #[arg(short = 'M', long = "memory-sync")]
        memory_sync: bool,

        /// Show what would be done without making changes.
        #[arg(short = 'n', long = "dry-run")]
        dry_run: bool,

        /// Additional repositories (can be repeated).
        #[arg(short = 'R', long = "repository", value_name = "URL")]
        repositories: Vec<String>,

        /// Full path to rootdir.
        #[arg(short = 'r', long, value_name = "DIR")]
        rootdir: Option<PathBuf>,

        /// Enable reproducible mode in pkgdb.
        #[arg(long = "reproducible")]
        reproducible: bool,

        /// Enable staged packages.
        #[arg(long = "staging")]
        staging: bool,

        /// Disable repository index sync (default is sync).
        #[arg(long = "no-sync")]
        no_sync: bool,

        /// Enable package update mode.
        #[arg(short = 'u', long = "update")]
        update: bool,

        /// Enable verbose xbps messages.
        #[arg(long = "xbps-verbose")]
        xbps_verbose: bool,

        /// Packages to install.
        pkgs: Vec<String>,

        /// Extra raw xbps-install args after `--`.
        #[arg(last = true, allow_hyphen_values = true)]
        xbps_args: Vec<String>,
    },

    /// Remove packages (xbps-remove).
    Rm {
        /// Assume yes.
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        /// Path to xbps confdir.
        #[arg(short = 'C', long = "config", value_name = "DIR")]
        config_dir: Option<PathBuf>,

        /// Path to xbps package cache.
        #[arg(short = 'c', long, value_name = "DIR")]
        cachedir: Option<PathBuf>,

        /// Enable xbps debug output.
        #[arg(short = 'd', long)]
        debug: bool,

        /// Force removal even with reverse dependencies.
        #[arg(short = 'F', long = "force-revdeps")]
        force_revdeps: bool,

        /// Force package files removal.
        #[arg(short = 'f', long)]
        force: bool,

        /// Show what would be removed without making changes.
        #[arg(short = 'n', long = "dry-run")]
        dry_run: bool,

        /// Clean outdated package cache entries (-O, repeat for stronger cleanup).
        #[arg(short = 'O', long = "clean-cache", action = ArgAction::Count)]
        clean_cache: u8,

        /// Also remove orphaned dependencies (-o).
        #[arg(short = 'o', long)]
        orphans: bool,

        /// Disable recursive dependency removal (default is recursive).
        #[arg(long = "no-recursive")]
        no_recursive: bool,

        /// Full path to rootdir.
        #[arg(short = 'r', long, value_name = "DIR")]
        rootdir: Option<PathBuf>,

        /// Enable verbose xbps messages.
        #[arg(long = "xbps-verbose")]
        xbps_verbose: bool,

        /// Packages to remove.
        pkgs: Vec<String>,

        /// Extra raw xbps-remove args after `--`.
        #[arg(last = true, allow_hyphen_values = true)]
        xbps_args: Vec<String>,
    },

    /// Update system packages and/or tracked source packages.
    ///
    /// Without --all: updates system only (xbps-install -Su).
    /// With --all: updates system AND all vx-tracked source packages.
    Up {
        /// Update system + all vx-tracked source packages.
        #[arg(short = 'a', long)]
        all: bool,

        /// Show the plan only; do not make changes.
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Force rebuild even if already at candidate version.
        #[arg(short = 'f', long)]
        force: bool,

        /// Assume yes.
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        /// Build from local checkout instead of upstream (default is upstream).
        #[arg(long)]
        local: bool,
    },

    /// void-packages / xbps-src source build operations.
    Src {
        #[command(subcommand)]
        cmd: SrcCmd,
    },

    /// Packaging helpers (template workflows).
    Pkg {
        /// Package name.
        name: Option<String>,

        /// Generate/update SHA256 checksums in template (xgensum -i).
        #[arg(long)]
        gensum: bool,

        /// Force re-download of distfiles (xgensum -f).
        #[arg(short = 'f', long)]
        force: bool,

        /// Use content checksum (xgensum -c).
        #[arg(short = 'c', long)]
        content: bool,

        /// Architecture (xgensum -a).
        #[arg(short = 'a', long, value_name = "ARCH")]
        arch: Option<String>,

        /// Absolute path to hostdir (xgensum -H).
        #[arg(short = 'H', long, value_name = "PATH")]
        hostdir: Option<PathBuf>,

        #[command(subcommand)]
        cmd: Option<PkgCmd>,
    },
}

#[derive(Subcommand, Debug)]
pub enum SrcCmd {
    /// Build + install a source package and start tracking it.
    ///
    /// Builds from upstream by default. Use --local for your checkout.
    Add {
        /// Assume yes.
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        /// Build from local checkout instead of upstream.
        #[arg(long)]
        local: bool,

        /// Packages to build and install.
        pkgs: Vec<String>,
    },

    /// Remove a source-built package and stop tracking it.
    Rm {
        /// Assume yes.
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        /// Packages to remove and untrack.
        pkgs: Vec<String>,
    },

    /// Rebuild and reinstall tracked source packages.
    ///
    /// With no arguments: rebuilds all tracked packages.
    /// With package names: rebuilds only those packages.
    ///
    /// Builds from upstream by default. Use --local for your checkout.
    Up {
        /// Show the plan only; do not make changes.
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Force rebuild even if already at candidate version.
        #[arg(short = 'f', long)]
        force: bool,

        /// Assume yes.
        #[arg(short = 'y', long, aliases = ["no-confirm", "noconfirm"])]
        yes: bool,

        /// Build from local checkout instead of upstream.
        #[arg(long)]
        local: bool,

        /// Packages to update (default: all tracked).
        pkgs: Vec<String>,
    },

    /// List tracked source packages.
    List,

    /// Build a source package without installing (./xbps-src pkg).
    Build {
        /// Build from local checkout instead of upstream.
        #[arg(long)]
        local: bool,

        pkgs: Vec<String>,
    },

    /// Clean build files (./xbps-src clean).
    Clean { pkgs: Vec<String> },

    /// Lint a template (./xbps-src lint).
    Lint { pkgs: Vec<String> },

    /// Search srcpkgs by name.
    Search {
        /// Only show packages that are installed.
        #[arg(short = 'i', long)]
        installed: bool,

        /// Name substring to search for.
        term: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum PkgCmd {
    /// Create a new template skeleton (xnew).
    New {
        /// Package name.
        name: String,
    },
}
