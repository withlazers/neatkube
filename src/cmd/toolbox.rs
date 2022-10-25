use crate::result::Result;
use crate::toolbox::tool::{Tool, VersionRef};
use crate::toolbox::Toolbox;
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(name = "toolbox", about = "manages the toolbox")]
pub struct ToolboxCommand {
    #[clap(subcommand)]
    pub subcommand: Subcommand,
}

#[derive(Parser, Debug)]
pub struct List {
    #[clap(short, long)]
    description: bool,
    tool: Option<String>,
}
impl List {
    async fn run(&self, toolbox: &Toolbox) -> Result<()> {
        if let Some(tool) = &self.tool {
            self.list_versions(toolbox, tool).await
        } else {
            self.list(toolbox).await
        }
    }
    async fn list(&self, toolbox: &Toolbox) -> Result<()> {
        let repository = toolbox.repository();
        let tools = repository.tools();
        let length = tools.iter().map(|x| x.name().len()).max().unwrap_or(0);

        for tool in tools {
            if self.description {
                println!("{:length$} {}", tool.name(), tool.description());
            } else {
                println!("{}", tool.name());
            }
        }
        Ok(())
    }

    async fn list_versions(&self, toolbox: &Toolbox, tool: &str) -> Result<()> {
        let tool = toolbox.tool(tool)?;
        let versions = tool.find_local_versions().await?;
        for version in &versions {
            println!("{}", version);
        }
        if versions.is_empty() {
            eprintln!("No Versions Installed");
        }
        Ok(())
    }
}
#[derive(Parser, Debug)]
pub struct Remote {
    tool: String,
}
impl Remote {
    async fn run(&self, toolbox: &Toolbox) -> Result<()> {
        let tool = toolbox.tool(&self.tool)?;

        println!("{}", tool.find_latest_version().await?);
        Ok(())
    }
}

#[derive(Parser, Debug)]
pub struct Install {
    tool: String,
    #[clap(short, long, action)]
    force: bool,
}
impl Install {
    async fn run(&self, toolbox: &Toolbox) -> Result<()> {
        let tool =
            toolbox.tool_with_version(&self.tool, vec![VersionRef::Latest])?;

        let name = tool.name().to_string();
        match tool.install(self.force).await? {
            true => println!("Installed: {}", name),
            false => println!("Already installed: {}", name),
        }
        Ok(())
    }
}

#[derive(Parser, Debug)]
pub struct Update {}
impl Update {
    async fn run(&self, toolbox: &Toolbox) -> Result<()> {
        let tools = toolbox.installed_tools().await?;
        for tool in tools {
            let tool =
                Tool::new_with_version(tool, toolbox, vec![VersionRef::Latest]);
            let name = tool.name().to_string();
            match tool.install(false).await? {
                true => println!("Updated: {}", name),
                false => println!("Already up to date: {}", name),
            }
        }
        Ok(())
    }
}

#[derive(Parser, Debug)]
pub struct Cleanup {}
impl Cleanup {
    async fn run(&self, toolbox: &Toolbox) -> Result<()> {
        let tools = toolbox.installed_tools().await?;
        for tool in tools {
            let tool =
                Tool::new_with_version(tool, toolbox, vec![VersionRef::Latest]);
            self.cleanup_tool(&tool).await?;
        }
        Ok(())
    }

    async fn cleanup_tool(&self, tool: &Tool<'_>) -> Result<()> {
        let all_versions = tool.find_local_versions().await?;
        let current_version = tool.find_local_version().await?;
        for version in all_versions {
            if Some(version.clone()) != current_version {
                continue;
            }

            let tool = Tool::new_with_version(
                tool.definition,
                tool.toolbox,
                vec![VersionRef::Specific(version)],
            );
            tool.remove().await?;
        }
        Ok(())
    }
}

#[derive(Parser, Debug)]
pub enum Subcommand {
    List(List),
    Update(Update),
    Remote(Remote),
    Install(Install),
    Cleanup(Cleanup),
}

impl ToolboxCommand {
    pub async fn run(self, toolbox: &Toolbox) -> Result<()> {
        match &self.subcommand {
            Subcommand::List(list) => list.run(toolbox).await,
            Subcommand::Remote(remote) => remote.run(toolbox).await,
            Subcommand::Update(update) => update.run(toolbox).await,
            Subcommand::Install(install) => install.run(toolbox).await,
            Subcommand::Cleanup(cleanup) => cleanup.run(toolbox).await,
        }
    }
}
