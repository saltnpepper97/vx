// Author Dustin Pilgrim
// License: MIT

use crate::config::Config;
use std::{env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct SrcResolved {
    pub voidpkgs: PathBuf,
    pub local_repo_rel: PathBuf,
    pub use_nonfree: bool,
}

pub fn resolve_voidpkgs(
    voidpkgs_override: Option<PathBuf>,
    cfg: Option<&Config>,
) -> Result<SrcResolved, String> {
    let mut local_repo_rel = PathBuf::from("hostdir/binpkgs");
    let mut use_nonfree = true;

    if let Some(c) = cfg {
        if !c.local_repo_rel.as_os_str().is_empty() {
            local_repo_rel = c.local_repo_rel.clone();
        }
        use_nonfree = c.use_nonfree;
    }

    if let Some(p) = voidpkgs_override {
        return Ok(SrcResolved {
            voidpkgs: p,
            local_repo_rel,
            use_nonfree,
        });
    }

    if let Ok(v) = env::var("VX_VOIDPKGS") {
        let p = PathBuf::from(v);
        if !p.as_os_str().is_empty() {
            return Ok(SrcResolved {
                voidpkgs: p,
                local_repo_rel,
                use_nonfree,
            });
        }
    }

    if let Some(c) = cfg {
        if let Some(p) = &c.void_packages_path {
            if !p.as_os_str().is_empty() {
                return Ok(SrcResolved {
                    voidpkgs: p.clone(),
                    local_repo_rel,
                    use_nonfree,
                });
            }
        }
    }

    Err(
        "vx src requires a void-packages path.\n\
         Provide one of:\n\
         - --voidpkgs /path/to/void-packages\n\
         - VX_VOIDPKGS=/path/to/void-packages\n\
         - ~/.config/vx/vx.rune with void_packages.path\n"
            .to_string(),
    )
}

