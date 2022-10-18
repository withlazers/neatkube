use clap::Parser;

use crate::{
    error::Error, podutil::file_transfer::PodFileTransfer, result::Result,
    toolbox::Toolbox,
};
use std::{convert::Infallible, result::Result as StdResult, str::FromStr};

#[derive(Parser, Debug)]
#[clap(name = "copy", about = "drops you to a temporary shell on a cluster")]
pub struct CopyCommand {
    /// namespace to use, default is infered
    #[clap(short, long, env = "NAMESPACE")]
    namespace: Option<String>,

    source: FileLocation,
    destination: FileLocation,
}

#[derive(Debug)]
enum FileLocation {
    Local { path: String },
    Remote { pod: String, path: String },
}
impl FromStr for FileLocation {
    type Err = Infallible;

    fn from_str(s: &str) -> StdResult<Self, Self::Err> {
        let volume = if let Some((host, path)) = &s.split_once(':') {
            FileLocation::Remote {
                pod: host.to_string(),
                path: path.to_string(),
            }
        } else {
            FileLocation::Local {
                path: s.to_string(),
            }
        };
        Ok(volume)
    }
}

impl CopyCommand {
    pub async fn run(&self, toolbox: &Toolbox) -> Result<()> {
        let file_transfer = PodFileTransfer::new(toolbox);
        let file_transfer = if let Some(namespace) = &self.namespace {
            file_transfer.namespace(namespace)
        } else {
            file_transfer
        };

        match (&self.source, &self.destination) {
            (
                FileLocation::Local { path: src },
                FileLocation::Remote { pod, path: dest },
            ) => {
                file_transfer.name(pod).upload(src, dest).await?;
            }
            (
                FileLocation::Remote { pod, path: src },
                FileLocation::Local { path: dest },
            ) => {
                file_transfer.name(pod).download(src, dest).await?;
            }
            _ => {
                Err::<_, Error>(
                    "Can only copy between local and remote volumes".into(),
                )?;
            }
        }
        Ok(())
    }
}
