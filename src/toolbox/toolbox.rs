use std::path::PathBuf;

use super::{repository::Repository, tool::Tool};
use crate::{dirs::dirs, result::Result};

static DEFAULT_NO_OPT_TOOL: &str = "k9s";
static DEFAULT_OPT_TOOL: &str = "kubectl";

static REPOSITORY: &'static [u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/repository.yaml"));

//static REPOSITORY_URL: &'static str = "https://neatkube.withlazers.dev/repository.yaml";

pub struct Toolbox {
    repository: Repository,
}

impl Toolbox {
    pub async fn create() -> Result<Self> {
        Ok(Self {
            repository: serde_yaml::from_slice(REPOSITORY)?,
        })
    }

    pub fn repository(&self) -> &Repository {
        &self.repository
    }

    pub fn bin_dir_path(&self) -> Result<PathBuf> {
        Ok(dirs()?.data_dir().join("bin"))
    }

    pub fn exec_dir_path(&self) -> Result<PathBuf> {
        Ok(dirs()?.data_dir().join("exec"))
    }

    pub async fn tool<'a>(&'a self, name: &str) -> Result<Tool<'a>> {
        self.repository
            .tools()
            .iter()
            .find(|t| t.name == name)
            .map(|t| Tool::new(t, &self))
            .ok_or(format!("Tool not found: {name}").into())
    }

    pub async fn run(self, args: &[&str]) -> Result<()> {
        let (tool, args) =
            if args.first().map(|a| a.starts_with("-")).unwrap_or(true) {
                (self.tool(DEFAULT_NO_OPT_TOOL).await?, args)
            } else {
                if let Ok(tool) = self.tool(args[0]).await {
                    (tool, &args[1..])
                } else {
                    (self.tool(DEFAULT_OPT_TOOL).await?, args)
                }
            };

        tool.run(args).await
    }
}

pub trait ToolboxMounter<'a> {
    fn mount_toolbox(self, toolbox: &'a Toolbox) -> clap::Command<'a>;
}

impl<'a> ToolboxMounter<'a> for clap::Command<'a> {
    fn mount_toolbox(self, toolbox: &'a Toolbox) -> clap::Command<'a> {
        let mut command = self;
        for tool in toolbox.repository().tools() {
            let tool = Tool::new(tool, &toolbox);
            command = command.subcommand(tool.subcommand());
        }

        command
    }
}
