use std::path::{Path, PathBuf};

use anyhow::Error;
use fuser::{Filesystem, MountOption};

struct CultivateFS {
    data_dir: PathBuf,
}

impl CultivateFS {
    pub fn new(data_dir: &Path) -> Self {
        CultivateFS {
            data_dir: data_dir.to_path_buf(),
        }
    }
}

pub fn mount(mountpoint: &Path, data_dir: &Path) -> Result<(), Error> {
    let options = vec![MountOption::FSName("cultivate".to_string())];
    fuser::mount2(CultivateFS::new(data_dir), mountpoint, &options)?;
    Ok(())
}

impl Filesystem for CultivateFS {}
