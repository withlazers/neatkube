use crate::result::Result;
use crate::toolbox::tool::VersionRef;
use crate::toolbox::Toolbox;
use clap::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "toolbox", about = "manages the toolbox")]
pub struct ToolboxCommand {
    #[structopt(subcommand)]
    pub subcommand: Subcommand,
}

#[derive(StructOpt, Debug)]
pub enum Subcommand {
    List {
        #[structopt(short, long)]
        description: bool,
        tool: Option<String>,
    },
    Remote {
        tool: String,
    },
    Install {
        tool: String,
        #[clap(short, long, action)]
        force: bool,
    },
}

impl ToolboxCommand {
    pub async fn run(self, toolbox: &Toolbox) -> Result<()> {
        match &self.subcommand {
            Subcommand::List {
                description,
                tool: None,
            } => self.list(toolbox, *description).await,
            Subcommand::List {
                tool: Some(tool), ..
            } => self.list_versions(toolbox, tool).await,
            Subcommand::Remote { tool } => self.remote(toolbox, tool).await,
            Subcommand::Install { tool, force } => {
                self.install(toolbox, tool, *force).await
            }
        }
    }

    async fn list(
        &self,
        toolbox: &Toolbox,
        show_description: bool,
    ) -> Result<()> {
        let repository = toolbox.repository();
        let tools = repository.tools();
        let length = tools.iter().map(|x| x.name.len()).max().unwrap_or(0);

        for tool in repository.tools() {
            if show_description {
                println!("{:length$} {}", tool.name, tool.description);
            } else {
                println!("{}", tool.name);
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

    async fn remote(&self, toolbox: &Toolbox, tool: &str) -> Result<()> {
        let tool = toolbox.tool(tool)?;

        println!("{}", tool.find_latest_version().await?);
        Ok(())
    }

    async fn install(
        &self,
        toolbox: &Toolbox,
        tool: &str,
        force: bool,
    ) -> Result<()> {
        let tool = toolbox.tool_with_version(tool, vec![VersionRef::Latest])?;

        let name = tool.name().to_string();
        match tool.install(force).await? {
            true => println!("Installed: {}", name),
            false => println!("Already installed: {}", name),
        }
        Ok(())
    }
}
