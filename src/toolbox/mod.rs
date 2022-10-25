mod repository;
pub mod tool;
mod upstream;

use std::path::PathBuf;

use crate::{dirs::Dirs, download::Downloader, result::Result};

use self::{
    repository::Repository,
    tool::{Tool, ToolDefinition, VersionRef},
};

static REPOSITORY: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/repository.yaml"));

//static REPOSITORY_URL: &'static str = "https://neatkube.withlazers.dev/repository.yaml";

pub struct Toolbox {
    repository: Repository,
    downloader: Downloader,
}

impl Toolbox {
    pub async fn create() -> Result<Self> {
        Ok(Self {
            repository: serde_yaml::from_slice(REPOSITORY)?,
            downloader: Downloader::default(),
        })
    }

    pub fn downloader(&self) -> &Downloader {
        &self.downloader
    }

    pub fn repository(&self) -> &Repository {
        &self.repository
    }

    pub fn bin_dir_path(&self) -> Result<PathBuf> {
        Ok(Dirs::data_dir()?.join("bin"))
    }

    pub fn exec_dir_path(&self) -> Result<PathBuf> {
        Ok(Dirs::data_dir()?.join("exec"))
    }

    pub async fn installed_tools(&self) -> Result<Vec<&ToolDefinition>> {
        let mut tools = Vec::new();
        for tool_definition in self.repository.tools() {
            let tool = Tool::new_with_version(
                tool_definition,
                self,
                vec![VersionRef::Local],
            );
            if tool.is_installed().await? {
                tools.push(tool_definition);
            }
        }
        Ok(tools)
    }

    pub fn tool_with_version<'a, I>(
        &'a self,
        name: &str,
        version_refs: I,
    ) -> Result<Tool<'a>>
    where
        I: IntoIterator<Item = VersionRef>,
    {
        self.repository
            .tools()
            .iter()
            .find(|t| t.name() == name)
            .map(|t| Tool::new_with_version(t, self, version_refs))
            .ok_or_else(|| format!("Tool not found: {name}").into())
    }

    pub fn tool<'a>(&'a self, name: &str) -> Result<Tool<'a>> {
        self.tool_with_version(name, [VersionRef::Local, VersionRef::Latest])
    }

    pub async fn mount_toolbox<'a>(
        &'a self,
        command: clap::Command<'a>,
    ) -> Result<clap::Command<'a>> {
        let mut command = command;
        for tool in self.installed_tools().await? {
            let tool = Tool::new(tool, self);
            command = command.subcommand(tool.subcommand());
        }

        Ok(command)
    }
}
