// Author Dustin Pilgrim
// License: MIT

use std::{
    collections::hash_map::DefaultHasher,
    env,
    fs,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Global default TTL for sync caches (in seconds).
/// Override with VX_SYNC_TTL_SECS.
pub const DEFAULT_SYNC_TTL_SECS: u64 = 600;

/// Force bypass all caches when set to "1"/"true"/"yes".
pub fn force_fresh() -> bool {
    match env::var("VX_FRESH") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true" || v == "yes"
        }
        Err(_) => false,
    }
}

/// TTL override for sync caches.
pub fn sync_ttl_secs() -> u64 {
    match env::var("VX_SYNC_TTL_SECS") {
        Ok(v) => v.trim().parse::<u64>().unwrap_or(DEFAULT_SYNC_TTL_SECS),
        Err(_) => DEFAULT_SYNC_TTL_SECS,
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn xdg_cache_home() -> PathBuf {
    if let Ok(v) = env::var("XDG_CACHE_HOME") {
        let p = PathBuf::from(v);
        if !p.as_os_str().is_empty() {
            return p;
        }
    }
    // fallback: ~/.cache
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cache")
}

/// ~/.cache/vx/...
fn vx_cache_dir() -> PathBuf {
    xdg_cache_home().join("vx")
}

fn ensure_dir(p: &Path) -> io::Result<()> {
    fs::create_dir_all(p)
}

fn key_path(key: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let h = hasher.finish();
    vx_cache_dir().join(format!("{:016x}.stamp", h))
}

/// True if the cache key was marked within ttl seconds.
pub fn is_fresh(key: &str, ttl_secs: u64) -> bool {
    if force_fresh() {
        return false;
    }

    let p = key_path(key);
    let data = match fs::read_to_string(&p) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let last = match data.trim().parse::<u64>() {
        Ok(v) => v,
        Err(_) => return false,
    };

    let now = now_secs();
    now.saturating_sub(last) <= ttl_secs
}

/// Mark a cache key as updated "now".
pub fn mark(key: &str) {
    let dir = vx_cache_dir();
    if ensure_dir(&dir).is_err() {
        return;
    }

    let p = key_path(key);
    let _ = fs::write(p, format!("{}", now_secs()));
}

