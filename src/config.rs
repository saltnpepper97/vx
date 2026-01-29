// Author Dustin Pilgrim
// License: MIT

use crate::paths::user_config_path;
use rune_cfg::RuneConfig;
use std::{
    fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct Config {
    pub debug: bool,

    /// Optional: if empty/None, caller should fall back to:
    ///   1) --voidpkgs
    ///   2) VX_VOIDPKGS env var
    ///   3) no config -> src commands error with instructions
    pub void_packages_path: Option<PathBuf>,

    /// Relative to void-packages root. Default: hostdir/binpkgs
    pub local_repo_rel: PathBuf,

    /// Use `.../nonfree` repo if present.
    pub use_nonfree: bool,
}

impl Config {
    /// Bootstrap behavior:
    /// - If config doesn't exist, ask ONCE (interactive) whether to create a default config at:
    ///     $HOME/.config/vx/vx.rune
    /// - If user says no, VX creates a sentinel so it won't ask again.
    ///
    /// NOTE: This uses stdin/stdout; keep it early in program startup.
    pub fn load_or_bootstrap_interactive() -> Result<Option<Self>, String> {
        let path = user_config_path()?;
        if path.exists() {
            return Self::from_file(&path).map(Some);
        }

        // Only prompt if stdin+stdout are terminals.
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Ok(None);
        }

        // Ask-once sentinel (if user previously said "no", do not prompt again).
        let sentinel = bootstrap_sentinel_path(&path)?;
        if sentinel.exists() {
            return Ok(None);
        }

        println!(
            "vx: no config found.\n\
             Create default config at {} ?\n\
             (You can also skip this and use VX_VOIDPKGS for `vx src ...`.)",
            path.display()
        );
        print!("Create config? [Y/n] ");
        let _ = io::stdout().flush();

        let mut s = String::new();
        let ok = io::stdin().read_line(&mut s).is_ok();
        let t = s.trim().to_ascii_lowercase();

        // If stdin read failed, do not create anything.
        if !ok {
            return Ok(None);
        }

        let yes = t.is_empty() || matches!(t.as_str(), "y" | "yes");
        if !yes {
            // Mark that we asked already so we don't nag on every run.
            write_bootstrap_sentinel(&sentinel)?;
            return Ok(None);
        }

        self::write_default_config(&path)?;
        Self::from_file(&path).map(Some)
    }

    fn from_file(path: &Path) -> Result<Self, String> {
        let cfg = RuneConfig::from_file(path.to_str().ok_or("invalid config path")?)
            .map_err(|e| format!("failed to parse config {}: {e}", path.display()))?;

        // base.debug (default false)
        let debug: bool = cfg.get("base.debug").unwrap_or(false);

        // void_packages.path (optional; empty means None)
        let void_packages_path_s: String = cfg
            .get("void_packages.path")
            .unwrap_or_else(|_| String::new());
        let void_packages_path = {
            let p = void_packages_path_s.trim();
            if p.is_empty() {
                None
            } else {
                Some(PathBuf::from(p))
            }
        };

        // void_packages.local_repo (default hostdir/binpkgs)
        let local_repo_rel_s: String = cfg
            .get("void_packages.local_repo")
            .unwrap_or_else(|_| "hostdir/binpkgs".into());
        let local_repo_rel = PathBuf::from(local_repo_rel_s);

        // void_packages.use_nonfree (default true)
        let use_nonfree: bool = cfg.get("void_packages.use_nonfree").unwrap_or(true);

        Ok(Self {
            debug,
            void_packages_path,
            local_repo_rel,
            use_nonfree,
        })
    }
}

fn bootstrap_sentinel_path(config_path: &Path) -> Result<PathBuf, String> {
    let dir = config_path
        .parent()
        .ok_or_else(|| format!("invalid config path: {}", config_path.display()))?;
    Ok(dir.join(".vx_bootstrap_asked"))
}

fn write_bootstrap_sentinel(path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .map_err(|e| format!("failed to create config dir {}: {e}", dir.display()))?;
    }
    fs::write(path, b"asked\n")
        .map_err(|e| format!("failed to write sentinel {}: {e}", path.display()))?;
    Ok(())
}

fn write_default_config(path: &Path) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| format!("invalid config path: {}", path.display()))?;

    fs::create_dir_all(dir)
        .map_err(|e| format!("failed to create config dir {}: {e}", dir.display()))?;

    let default = default_config_text();

    fs::write(path, default)
        .map_err(|e| format!("failed to write config {}: {e}", path.display()))?;

    println!("vx: wrote default config: {}", path.display());
    Ok(())
}

fn default_config_text() -> String {
    // Keep this aligned with the shipped example config.
    // Intentionally does NOT hard-require void_packages.path because VX supports VX_VOIDPKGS / --voidpkgs.
    r#"@author "Dustin Pilgrim"
@description "Unified Void package manager config (xbps + void-packages)"

base:
  debug false
end

# Optional. Only needed if you want `vx src ...` without setting VX_VOIDPKGS or using --voidpkgs.
void_packages:
  # Uncomment and set this variable to the location of your void-packages directory
  #path "$env.HOME/void-packages"

  # relative to void-packages root
  local_repo "hostdir/binpkgs"

  # if true, and a `nonfree/` repo exists under local_repo, VX will add it as -R too
  use_nonfree true
end
"#
    .to_string()
}

