use crate::result::Result;

use crate::toolbox::tool::Tool;
use crate::toolbox::Toolbox;

pub struct ToolCommand {
    args: Vec<String>,
}

impl ToolCommand {
    pub fn from_arg_matches(matches: &clap::ArgMatches) -> Result<Self> {
        let args: Vec<String> = matches
            .get_many::<String>("")
            .map(|x| x.cloned().collect())
            .unwrap_or_default();
        Ok(Self { args })
    }
    pub async fn run(self, tool: &str, toolbox: Toolbox) -> Result<()> {
        let tool = match toolbox.tool(tool).await {
            Ok(tool) => tool,
            Err(_) => return self.run_default(tool, toolbox).await,
        };
        tool.run(&self.args.iter().map(String::as_str).collect::<Vec<_>>())
            .await?;
        Ok(())
    }

    async fn run_default(
        self,
        tool_name: &str,
        toolbox: Toolbox,
    ) -> Result<()> {
        let tool = if tool_name.starts_with("-") {
            toolbox.tool("k9s").await?
        } else {
            toolbox.tool("kubectl").await?
        };
        let mut args = vec![tool_name];
        args.extend(self.args.iter().map(String::as_str));

        tool.run(&args).await?;
        Ok(())
    }
}
