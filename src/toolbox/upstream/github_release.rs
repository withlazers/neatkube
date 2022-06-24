use serde::Deserialize;

use super::Upstream;
use crate::{error::Error, result::Result};

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct GithubReleaseUpstream {
    repo: String,
    #[serde(flatten)]
    source: PackageSource,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum PackageSource {
    File(String),
    PackageUrl(String),
}

impl Upstream for GithubReleaseUpstream {
    fn version_url(&self) -> String {
        format!("https://api.github.com/repos/{}/releases/latest", self.repo)
    }

    fn package_url(&self, version: &str) -> String {
        match &self.source {
            PackageSource::File(file) => {
                format!(
                    "https://github.com/{}/releases/download/{}/{}",
                    self.repo, version, file
                )
            }
            PackageSource::PackageUrl(url) => url.replace("{version}", version),
        }
    }

    fn parse_version_from_response(&self, response: &str) -> Result<String> {
        let json: serde_yaml::Value = serde_yaml::from_str(response)
            .or(Err(response.trim().to_string()))?;
        let tag_name = json["tag_name"]
            .as_str()
            .ok_or::<Error>("Malformed response".into())?;
        Ok(tag_name.to_string())
    }
}
