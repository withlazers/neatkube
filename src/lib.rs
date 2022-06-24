pub mod cmd;
pub mod dirs;
pub mod download;
pub mod error;
pub mod result;
pub mod toolbox;
use clap::Command;
use clap::FromArgMatches;
use clap::IntoApp;
use cmd::cfg_pack::CfgPackCommand;
use cmd::tool::ToolCommand;
use cmd::toolbox::ToolboxCommand;
use toolbox::Toolbox;
use toolbox::ToolboxMounter;

pub struct Opt;

pub async fn run() -> Result<(), error::Error> {
    let toolbox = Toolbox::create().await?;

    let command = Command::new("nk")
        .allow_external_subcommands(true)
        .subcommand(CfgPackCommand::command())
        .subcommand(ToolboxCommand::command())
        .allow_hyphen_values(true)
        .mount_toolbox(&toolbox);

    let matches = command.get_matches();
    match matches.subcommand() {
        Some(("cfgpack", subcommand)) => {
            CfgPackCommand::from_arg_matches(subcommand)?.run().await?
        }
        Some(("toolbox", subcommand)) => {
            ToolboxCommand::from_arg_matches(subcommand)?
                .run(toolbox)
                .await?
        }
        Some((name, subcommand)) => {
            ToolCommand::from_arg_matches(subcommand)?
                .run(name, toolbox)
                .await?
        }
        _ => {
            ToolCommand::from_arg_matches(&matches)?
                .run("k9s", toolbox)
                .await?
        }
    }
    Ok(())
}
