pub mod cmd;
pub mod completion;
pub mod dirs;
pub mod download;
pub mod error;
pub mod result;
pub mod toolbox;

use clap::Command;
use clap::FromArgMatches;
use clap::IntoApp;
use cmd::cfg_pack::CfgPackCommand;
use cmd::completion::CompletionCommand;
use cmd::shell::ShellCommand;
use cmd::tool::ToolCommand;
use cmd::toolbox::ToolboxCommand;
use toolbox::Toolbox;

pub struct Opt;

pub async fn run() -> Result<(), error::Error> {
    let toolbox = Toolbox::create().await?;

    let command = Command::new(env!("CARGO_PKG_NAME"))
        .bin_name("nk")
        .allow_external_subcommands(true)
        .subcommand(CfgPackCommand::command())
        .subcommand(ToolboxCommand::command())
        .subcommand(CompletionCommand::command())
        .subcommand(ShellCommand::command())
        .allow_hyphen_values(true);

    let command = toolbox.mount_toolbox(command).await?;

    let matches = command.clone().get_matches();
    match matches.subcommand() {
        Some(("shell", subcommand)) => {
            ShellCommand::from_arg_matches(subcommand)?
                .run(&toolbox)
                .await?
        }
        Some(("cfgpack", subcommand)) => {
            CfgPackCommand::from_arg_matches(subcommand)?.run().await?
        }
        Some(("completion", subcommand)) => {
            CompletionCommand::from_arg_matches(subcommand)?
                .run(&command, &toolbox)?;
        }
        Some(("toolbox", subcommand)) => {
            ToolboxCommand::from_arg_matches(subcommand)?
                .run(&toolbox)
                .await?
        }
        subcommand => ToolCommand::new(&toolbox, subcommand)?.run().await?,
    }
    Ok(())
}
