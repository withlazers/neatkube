use std::io;

use crate::{
    completion::{bash::Bash, zsh::Zsh},
    result::Result,
    toolbox::Toolbox,
};
use clap::{Command, StructOpt, ValueEnum};
use clap_complete::Generator;

#[derive(StructOpt, Debug)]
#[structopt(
    name = "completion",
    about = "Generate autocompletion scripts for a specified shell."
)]
pub struct CompletionCommand {
    #[clap(value_enum, help = "shell to generate completion scripts for")]
    pub shell: Shell,
}
#[derive(ValueEnum, Clone, Debug)]
pub enum Shell {
    Bash,
    Zsh,
}

impl CompletionCommand {
    pub fn run(self, command: &Command, toolbox: &Toolbox) -> Result<()> {
        let shell = self.shell;
        let mut out = io::stdout();
        match shell {
            Shell::Bash => {
                Bash::new(toolbox).generate(command, &mut out);
            }
            Shell::Zsh => {
                Zsh::new(toolbox).generate(command, &mut out);
            }
        }
        Ok(())
    }
}
