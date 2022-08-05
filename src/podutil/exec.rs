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
        let name = &self.name.as_ref().unwrap();
        let prog_args = args.into_iter().map(|s| s.as_ref().to_os_string());
        let mut args = vec!["exec", self.opt];
        if let Some(container_name) = &self.container_name {
            args.append(&mut vec!["-c", container_name]);
        }
        if let Some(namespace) = &self.namespace {
            args.append(&mut vec!["--namespace", namespace]);
        }
        args.append(&mut vec![name, "--"]);
        let args = args.into_iter().map(OsString::from).chain(prog_args);

        kubectl.command(args).await
    }
}
