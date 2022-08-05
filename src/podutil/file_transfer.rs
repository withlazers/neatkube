use std::{
    path::Path,
    process::{ExitStatus, Stdio},
};

use tokio::{io, process::Command};

use crate::toolbox::Toolbox;

use super::exec::PodExec;
use crate::result::Result;

static COMPRESSION: &str = "-z";

pub struct PodFileTransfer<'a> {
    pod_exec: PodExec<'a>,
}

pub struct TarInfo {
    strip_components: usize,
}

impl TarInfo {
    pub fn new(strip_components: usize) -> Self {
        TarInfo { strip_components }
    }

    pub fn strip_components(&self) -> usize {
        self.strip_components
    }
}

impl<'a> PodFileTransfer<'a> {
    pub fn new(toolbox: &'a Toolbox) -> PodFileTransfer {
        PodFileTransfer {
            pod_exec: PodExec::new(toolbox).terminal(false),
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.pod_exec = self.pod_exec.name(name);
        self
    }

    pub fn container_name(mut self, name: &str) -> Self {
        self.pod_exec = self.pod_exec.container_name(name);
        self
    }

    pub fn namespace(mut self, namespace: &str) -> Self {
        self.pod_exec = self.pod_exec.namespace(namespace);
        self
    }

    fn receive_args(&self, remote: &str, info: TarInfo) -> Vec<String> {
        let receive_script = format!(
            r#"mkdir -p "$1" && exec tar --strip-components {} -C "$1" {} -xv"#,
            info.strip_components(),
            COMPRESSION
        );
        ["/bin/sh", "-c", &receive_script, "-", remote]
            .into_iter()
            .map(String::from)
            .collect()
    }

    fn send_args(&self, local: &str) -> (TarInfo, Vec<String>) {
        let local = Path::new(local);
        let (working_dir, local) = if local.is_absolute() {
            ("/", local.strip_prefix("/").unwrap())
        } else {
            (".", local)
        };

        (
            TarInfo::new(local.components().count() - 1),
            vec![
                "tar",
                "-C",
                working_dir,
                "-c",
                COMPRESSION,
                local.to_str().unwrap(),
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        )
    }

    pub async fn upload(self, local: &str, remote: &str) -> Result<()> {
        let (info, args) = self.send_args(local);
        let mut send_cmd = Command::new(&args[0]);
        send_cmd.args(&args[1..]);

        let receive_cmd = self
            .pod_exec
            .command(&self.receive_args(remote, info))
            .await?;

        let (tar_exit, kubectl_exit) =
            self.transfer(send_cmd, receive_cmd).await?;

        if !tar_exit.success() {
            Err("tar failed".to_string())?
        }
        if !kubectl_exit.success() {
            Err("kubectl failed".to_string())?
        }
        Ok(())
    }

    pub async fn download(self, remote: &str, local: &str) -> Result<()> {
        let (info, args) = self.send_args(remote);
        let send_cmd = self
            .pod_exec
            .command(args)
            .await?;

        let args = self.receive_args(local, info);
        let mut receive_cmd = Command::new(&args[0]);
        receive_cmd.args(&args[1..]);

        let (kubectl_exit, tar_exit) =
            self.transfer(send_cmd, receive_cmd).await?;

        if !kubectl_exit.success() {
            Err("kubectl failed".to_string())?
        }
        if !tar_exit.success() {
            Err("tar failed".to_string())?
        }
        Ok(())
    }

    async fn transfer(
        self,
        mut send_cmd: Command,
        mut receive_cmd: Command,
    ) -> Result<(ExitStatus, ExitStatus)> {
        let mut send_child = send_cmd.stdout(Stdio::piped()).spawn()?;
        let mut receive_child = receive_cmd.stdin(Stdio::piped()).spawn()?;

        io::copy(
            send_child.stdout.as_mut().unwrap(),
            receive_child.stdin.as_mut().unwrap(),
        )
        .await?;
        Ok(tokio::try_join!(send_child.wait(), receive_child.wait())?)
    }
}
