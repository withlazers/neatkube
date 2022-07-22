mod github_release;
mod simple;

use crate::result::Result;
pub use github_release::GithubReleaseUpstream;
use serde::Deserialize;
pub use simple::SimpleUpstream;

pub trait Upstream {
    fn version_url(&self) -> String;
    fn package_url(&self) -> String;
    fn parse_version_from_response(&self, response: &str) -> Result<String>;
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum UpstreamDefinition {
    GithubRelease(GithubReleaseUpstream),
    Simple(SimpleUpstream),
}
