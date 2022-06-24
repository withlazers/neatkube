use crate::result::Result;

pub fn dirs() -> Result<directories::ProjectDirs> {
    directories::ProjectDirs::from("dev", "withlazers", "neatkube")
        .ok_or("Failed to get project dirs".into())
}
