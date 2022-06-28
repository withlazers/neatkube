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

impl UpstreamDefinition {
    pub fn version_url(&self) -> String {
        match self {
            UpstreamDefinition::GithubRelease(upstream) => {
                upstream.version_url()
            }
            UpstreamDefinition::Simple(upstream) => upstream.version_url(),
        }
    }

    pub fn package_url(&self, version: &str) -> String {
        let url = match self {
            UpstreamDefinition::GithubRelease(upstream) => {
                upstream.package_url()
            }
            UpstreamDefinition::Simple(upstream) => upstream.package_url(),
        };

        url.replace("{version}", version).replace(
            "{stripped_version}",
            version.strip_prefix('v').unwrap_or(version),
        )
    }

    pub fn parse_version_from_response(
        &self,
        response: &str,
    ) -> Result<String> {
        match self {
            UpstreamDefinition::GithubRelease(upstream) => {
                upstream.parse_version_from_response(response)
            }
            UpstreamDefinition::Simple(upstream) => {
                upstream.parse_version_from_response(response)
            }
        }
    }
}
