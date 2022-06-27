use serde::Deserialize;

use super::Upstream;
use crate::result::Result;

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SimpleUpstream {
    version_url: String,
    package_url: String,
}

impl Upstream for SimpleUpstream {
    fn version_url(&self) -> String {
        self.version_url.clone()
    }

    fn package_url(&self) -> String {
        self.package_url.to_string()
    }

    fn parse_version_from_response(&self, response: &str) -> Result<String> {
        Ok(response.trim().to_string())
    }
}
