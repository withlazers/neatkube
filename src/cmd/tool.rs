use std::iter;

use crate::result::Result;

use crate::toolbox::tool::Tool;
use crate::toolbox::Toolbox;

pub struct ToolCommand<'a> {
    args: Vec<String>,
    toolbox: &'a Toolbox,
    tool_name: String,
}

impl<'a> ToolCommand<'a> {
    pub async fn run(self) -> Result<()> {
        let tool = self.toolbox.tool(&self.tool_name)?;

        tool.run(self.args).await?;
        Ok(())
    }

    pub fn new(
        toolbox: &'a Toolbox,
        subcommand: Option<(&str, &clap::ArgMatches)>,
    ) -> Result<ToolCommand<'a>> {
        let default_no_subcommand =
            toolbox.repository().default_no_subcommand();
        let default_with_subcommand =
            toolbox.repository().default_with_subcommand();

        let (subcommand_name, matches) = match subcommand {
            None => {
                return Ok(Self {
                    args: vec![],
                    tool_name: default_no_subcommand.to_string(),
                    toolbox,
                })
            }
            Some(sc) => (sc),
        };

        if toolbox.tool(subcommand_name).is_ok() {
            return Ok(Self {
                args: Tool::get_args(matches).map(String::from).collect(),
                toolbox,
                tool_name: subcommand_name.to_string(),
            });
        }

        let tool_name = if subcommand_name.starts_with('-') {
            default_no_subcommand
        } else {
            default_with_subcommand
        }
        .to_string();

        let args = iter::once(subcommand_name)
            .chain(Tool::get_args(matches))
            .map(String::from)
            .collect();

        Ok(Self {
            args,
            toolbox,
            tool_name,
        })
    }
}
