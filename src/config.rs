// Author Dustin Pilgrim
// License: MIT

use crate::paths::user_config_path;
use rune_cfg::RuneConfig;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    #[allow(dead_code)]
    pub debug: bool,

    pub xbps_sudo: bool,
    pub xbps_install: String,
    pub xbps_remove: String,
    pub xbps_query: String,

    pub void_packages_path: PathBuf,
    pub local_repo_rel: PathBuf,
    pub use_nonfree: bool,
}

impl Config {
    /// Load user config if present. Config is optional.
    pub fn load() -> Result<Option<Self>, String> {
        let path = user_config_path()?;
        if !path.exists() {
            return Ok(None);
        }
        Self::from_file(&path).map(Some)
    }

    fn from_file(path: &Path) -> Result<Self, String> {
        let cfg = RuneConfig::from_file(path.to_str().ok_or("invalid config path")?)
            .map_err(|e| format!("failed to parse config {}: {e}", path.display()))?;

        // Top-level simple fields
        let debug: bool = cfg.get("debug").unwrap_or(false);

        // xbps section
        let xbps_sudo: bool = cfg.get("xbps.sudo").unwrap_or(true);
        let xbps_install: String = cfg
            .get("xbps.install")
            .unwrap_or_else(|_| "xbps-install".into());
        let xbps_remove: String = cfg
            .get("xbps.remove")
            .unwrap_or_else(|_| "xbps-remove".into());
        let xbps_query: String = cfg
            .get("xbps.query")
            .unwrap_or_else(|_| "xbps-query".into());

        // void_packages section
        let void_packages_path_s: String = cfg
            .get("void_packages.path")
            .unwrap_or_else(|_| String::new());
        let void_packages_path = PathBuf::from(void_packages_path_s);

        let local_repo_rel_s: String = cfg
            .get("void_packages.local_repo")
            .unwrap_or_else(|_| "hostdir/binpkgs".into());
        let local_repo_rel = PathBuf::from(local_repo_rel_s);

        let use_nonfree: bool = cfg.get("void_packages.use_nonfree").unwrap_or(true);

        Ok(Self {
            debug,
            xbps_sudo,
            xbps_install,
            xbps_remove,
            xbps_query,
            void_packages_path,
            local_repo_rel,
            use_nonfree,
        })
    }
}

