use std::ffi::{OsStr, OsString};

use crate::result::Result;
use tokio::process::Command;

use crate::toolbox::Toolbox;

pub struct PodExec<'a> {
    toolbox: &'a Toolbox,
    opt: &'static str,
    name: Option<String>,
    namespace: Option<String>,
    container_name: Option<String>,
}

impl<'a> PodExec<'a> {
    pub fn new(toolbox: &'a Toolbox) -> PodExec {
        PodExec {
            toolbox,
            opt: "-i",
            name: None,
            namespace: None,
            container_name: None,
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn namespace(mut self, namespace: &str) -> Self {
        self.namespace = Some(namespace.to_string());
        self
    }

    pub fn container_name(mut self, container_name: &str) -> Self {
        self.container_name = Some(container_name.to_string());
        self
    }

    pub fn terminal(mut self, is_terminal: bool) -> Self {
        if is_terminal {
            self.opt = "-ti";
        } else {
            self.opt = "-i";
        }
        self
    }

    pub async fn command<I, S>(&self, args: I) -> Result<Command>
    where
        S: AsRef<OsStr>,
        I: IntoIterator<Item = S>,
    {
        let kubectl = self.toolbox.tool("kubectl")?;
        let container_name = &self.container_name.as_ref().unwrap();
        let namespace = &self.namespace.as_ref().unwrap();
        let name = &self.name.as_ref().unwrap();
        let prog_args = args.into_iter().map(|s| s.as_ref().to_os_string());
        let args = [
            "exec",
            self.opt,
            "-c",
            container_name,
            "--namespace",
            namespace,
            name,
            "--",
        ]
        .into_iter()
        .map(OsString::from)
        .chain(prog_args);

        kubectl.command(args).await
    }
}
