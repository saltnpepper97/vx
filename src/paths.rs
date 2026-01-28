// Author Dustin Pilgrim
// License: MIT

use std::path::PathBuf;

pub fn user_config_path() -> Result<PathBuf, String> {
    let base = dirs::config_dir().ok_or("could not locate config dir")?;
    Ok(base.join("vx").join("vx.rune"))
}

pub fn managed_src_path() -> Result<PathBuf, String> {
    let base = dirs::config_dir().ok_or("could not locate config dir")?;
    Ok(base.join("vx").join("managed-src.rune"))
}

