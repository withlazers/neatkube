mod github_release;
mod simple;

use std::collections::HashMap;

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
pub struct UpstreamDefinition {
    #[serde(flatten)]
    pub upstream_impl: UpstreamImpl,
    #[serde(default)]
    pub os_map: HashMap<String, String>,
    #[serde(default)]
    pub arch_map: HashMap<String, String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum UpstreamImpl {
    GithubRelease(GithubReleaseUpstream),
    Simple(SimpleUpstream),
}

fn default_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "x86" => "386",
        "aarch64" => "arm64",
        x => x,
    }
}

fn default_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        x => x,
    }
}

impl UpstreamDefinition {
    pub fn version_url(&self) -> String {
        match &self.upstream_impl {
            UpstreamImpl::GithubRelease(upstream) => upstream.version_url(),
            UpstreamImpl::Simple(upstream) => upstream.version_url(),
        }
    }

    pub fn os(&self) -> &str {
        self.os_map
            .get(default_os())
            .map(String::as_str)
            .unwrap_or_else(|| default_os())
    }

    pub fn arch(&self) -> &str {
        self.arch_map
            .get(default_arch())
            .map(String::as_str)
            .unwrap_or_else(|| default_arch())
    }

    pub fn package_url(&self, version: &str) -> String {
        let url = match &self.upstream_impl {
            UpstreamImpl::GithubRelease(upstream) => upstream.package_url(),
            UpstreamImpl::Simple(upstream) => upstream.package_url(),
        };

        url.replace("{arch}", self.arch())
            .replace("{os}", self.os())
            .replace("{version}", version)
            .replace(
                "{stripped_version}",
                version.strip_prefix('v').unwrap_or(version),
            )
    }

    pub fn parse_version_from_response(
        &self,
        response: &str,
    ) -> Result<String> {
        match &self.upstream_impl {
            UpstreamImpl::GithubRelease(upstream) => {
                upstream.parse_version_from_response(response)
            }
            UpstreamImpl::Simple(upstream) => {
                upstream.parse_version_from_response(response)
            }
        }
    }
}
