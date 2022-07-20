use std::{path::Path, process::Stdio};

use tokio::{io, process::Command};

use crate::toolbox::Toolbox;

use super::exec::PodExec;
use crate::result::Result;

pub struct PodUpload<'a> {
    pod_exec: PodExec<'a>,
}

impl<'a> PodUpload<'a> {
    pub fn new(toolbox: &'a Toolbox) -> PodUpload {
        PodUpload {
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

    pub async fn upload(self, local: &str, remote: &str) -> Result<()> {
        let compression = "-z";
        let script =
            format!(r#"mkdir -p "$1" && exec tar -C "$1" {} -xv"#, compression);
        let receive_args = vec!["/bin/sh", "-c", script.as_str(), "-", remote];

        eprintln!("Uploading {} to {}", local, remote);
        let path = Path::new(local);
        let (dir, file) = if path.is_dir() {
            (path, ".")
        } else {
            (
                path.parent().unwrap_or_else(|| Path::new(".")),
                path.file_name().unwrap().to_str().unwrap(),
            )
        };

        let mut tar_cmd = Command::new("tar")
            .args(&["-C", dir.to_str().unwrap(), "-c", compression, file])
            .stdout(Stdio::piped())
            .spawn()?;

        let mut kubectl_cmd = self
            .pod_exec
            .command(receive_args)
            .await?
            .stdin(Stdio::piped())
            .spawn()?;
        io::copy(
            tar_cmd.stdout.as_mut().unwrap(),
            kubectl_cmd.stdin.as_mut().unwrap(),
        )
        .await?;
        let (tar_exit, kubectl_exit) =
            tokio::try_join!(tar_cmd.wait(), kubectl_cmd.wait())?;

        if !tar_exit.success() {
            Err("tar failed".to_string())?
        }
        if !kubectl_exit.success() {
            Err("kubectl failed".to_string())?
        }
        Ok(())
    }
}
