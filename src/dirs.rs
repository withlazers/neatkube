use std::path::PathBuf;

use crate::result::Result;

pub struct Dirs;

impl Dirs {
    pub fn data_dir() -> Result<PathBuf> {
        match std::env::var("NK_DATA_DIR") {
            Ok(path) => Ok(PathBuf::from(path)),
            _ => Ok(Self::dirs()?.data_dir().to_path_buf()),
        }
    }
    fn dirs() -> Result<directories::ProjectDirs> {
        directories::ProjectDirs::from("dev", "withlazers", "neatkube")
            .ok_or("Failed to get project dirs".into())
    }
}
