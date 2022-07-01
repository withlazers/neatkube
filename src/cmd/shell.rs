use std::{convert::Infallible, iter, process::Stdio};

use crate::{
    error::TError,
    result::Result,
    toolbox::{tool::Tool, Toolbox},
};
use clap::StructOpt;
use k8s_openapi::api::core::v1::{
    ConfigMapVolumeSource, Container, HostPathVolumeSource, Pod, PodSpec,
    SecretVolumeSource, SecurityContext, Volume, VolumeMount,
};
use kube_client::{
    api::{DeleteParams, ListParams, PostParams},
    core::{ObjectMeta, WatchEvent},
    Api, Client, Config, ResourceExt,
};
use log::info;
use std::result::Result as StdResult;
use tokio::{io, process::Command};

use futures::{StreamExt, TryStreamExt};

#[derive(StructOpt, Debug)]
#[structopt(
    name = "shell",
    about = "drops you to a temporary shell on a cluster"
)]
pub struct ShellCommand {
    /// container image to start
    #[clap(short, long, default_value = concat!("withlazers/", env!("CARGO_PKG_NAME"), ":v", env!("CARGO_PKG_VERSION")))]
    image: String,

    /// namespace to use, default is infered
    #[clap(short, long, env = "NAMESPACE")]
    namespace: Option<String>,

    /// node to run on
    #[clap(short = 'm', long)]
    node: Option<String>,

    /// share network namespace with host
    #[clap(short = 'N', long, action)]
    host_network: bool,

    /// share ipc namespace with host
    #[clap(short = 'I', long, action)]
    host_ipc: bool,

    /// share pid namespace with host
    #[clap(short = 'P', long, action)]
    host_pid: bool,

    /// start container in privileged mode
    #[clap(short, long, action)]
    privileged: bool,

    /// service account to use
    #[clap(short = 'a', long, name = "ACCOUNT]")]
    service_account: Option<String>,

    /// mounts a secret. if no path is given, the secret will be mounted at /secret
    #[clap(short, long = "secret", value_parser = volume_parser, name = "SECRET[:PATH]")]
    secrets: Vec<(String, Option<String>)>,

    /// mounts the host
    #[clap(short = 'H', long, value_parser = volume_parser, name = "HPATH[:PATH]")]
    hostdir: Vec<(String, Option<String>)>,

    #[clap(short, long, value_parser = volume_parser, name = "CMAP[:PATH]")]
    config_maps: Vec<(String, Option<String>)>,

    #[clap(short, long, value_parser = upload_parser, name = "LOCAL:PATH")]
    upload: Option<(String, String)>,

    #[clap(default_value = "/bin/sh")]
    args: Vec<String>,
}

fn upload_parser(input: &str) -> StdResult<(String, String), TError> {
    let (local, path) = volume_parser(input)?;
    Ok((
        local,
        path.ok_or_else(|| {
            "upload directories must be in the form of LOCAL:PATH".to_string()
        })?,
    ))
}

fn volume_parser(
    input: &str,
) -> StdResult<(String, Option<String>), Infallible> {
    let volume = match &input.split_once(':') {
        Some((name, path)) => (name.to_string(), path.to_string().into()),
        None => (input.to_string(), None),
    };
    Ok(volume)
}

impl ShellCommand {
    fn gen_volumes(
        kind: &'static str,
        volumes: &[(String, Option<String>)],
    ) -> Vec<(String, String, String)> {
        volumes
            .iter()
            .enumerate()
            .map(move |(i, (name, path))| {
                (
                    format!("{}-{}", kind, i),
                    name.clone(),
                    path.clone().unwrap_or_else(|| format!("/{kind}/{name}")),
                )
            })
            .collect()
    }
    pub async fn run(self, toolbox: &Toolbox) -> Result<()> {
        let config = Config::infer().await?;
        let client = Client::try_from(config.clone())?;
        let namespace = self
            .namespace
            .as_ref()
            .unwrap_or_else(|| &config.default_namespace)
            .clone();
        let pod_name = format!(
            "{}-shell-{}",
            env!("CARGO_PKG_NAME"),
            random_string::generate(6, "abcdefghijklmnopqrstuvwxyz1234567890")
        );
        let container_name = env!("CARGO_PKG_NAME");

        let command: Vec<_> = ["/bin/sleep", "infinity"]
            .into_iter()
            .map(String::from)
            .collect();

        let secrets = Self::gen_volumes("secret", &self.secrets);
        let config_maps = Self::gen_volumes("configmap", &self.config_maps);
        let host_dirs = Self::gen_volumes("host", &self.hostdir);

        let volumes: Vec<_> = iter::empty()
            .chain(secrets.iter().map(|(volume_name, name, _)| {
                Volume {
                    name: volume_name.clone(),
                    secret: SecretVolumeSource {
                        secret_name: name.clone().into(),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                }
            }))
            .chain(config_maps.iter().map(|(volume_name, name, _)| {
                Volume {
                    name: volume_name.clone(),
                    config_map: ConfigMapVolumeSource {
                        name: name.clone().into(),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                }
            }))
            .chain(host_dirs.iter().map(|(volumen_name, host_path, _)| {
                Volume {
                    name: volumen_name.clone(),
                    host_path: HostPathVolumeSource {
                        path: host_path.clone(),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                }
            }))
            .collect();

        let volume_mounts: Vec<_> = iter::empty()
            .chain(secrets)
            .chain(config_maps)
            .chain(host_dirs)
            .map(|(volume_name, _, path)| VolumeMount {
                name: volume_name,
                mount_path: path,
                ..Default::default()
            })
            .collect();

        let node_selector = self.node.as_ref().map(|node| {
            [("kubernetes.io/hostname".to_string(), node.clone())].into()
        });

        let pod = Pod {
            metadata: ObjectMeta {
                name: pod_name.clone().into(),
                ..Default::default()
            },
            spec: PodSpec {
                service_account_name: self.service_account.clone(),
                host_ipc: self.host_ipc.into(),
                host_network: self.host_network.into(),
                host_pid: self.host_pid.into(),
                containers: vec![Container {
                    name: container_name.to_string(),
                    image: self.image.clone().into(),
                    command: command.into(),
                    security_context: SecurityContext {
                        privileged: Some(self.privileged),
                        ..Default::default()
                    }
                    .into(),
                    volume_mounts: volume_mounts.into(),
                    ..Default::default()
                }],
                volumes: volumes.into(),
                node_selector,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };

        let pods: Api<Pod> = Api::namespaced(client, &namespace);
        pods.create(&PostParams::default(), &pod).await?;

        let lp = ListParams::default()
            .fields(&format!("metadata.name={}", pod.name()));
        let mut stream = pods.watch(&lp, "0").await?.boxed();
        while let Some(status) = stream.try_next().await? {
            match status {
                WatchEvent::Added(o) => {
                    info!("Added {}", o.name());
                }
                WatchEvent::Modified(o) => {
                    let s = o.status.as_ref().expect("status exists on pod");
                    if s.phase.clone().unwrap_or_default() == "Running" {
                        info!("Ready to attach to {}", o.name());
                        break;
                    }
                }
                _ => {}
            }
        }

        let kubectl = toolbox.tool("kubectl")?;
        let default_args: Vec<_> = [
            "exec",
            "-it",
            "-c",
            container_name,
            "--namespace",
            &namespace,
            &pod_name,
            "--",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        for upload in self.upload.iter() {
            self.upload(&kubectl, &default_args, upload).await?;
        }

        let args = default_args.iter().chain(self.args.iter());
        kubectl.command(args).await?.spawn()?.wait().await?;

        // Delete it
        pods.delete(&pod.name(), &DeleteParams::default())
            .await?
            .map_left(|pdel| {
                assert_eq!(pdel.name(), pod.metadata.name.unwrap());
            });

        Ok(())
    }

    async fn upload(
        &self,
        kubectl: &Tool<'_>,
        args: &[String],
        upload: &(String, String),
    ) -> Result<()> {
        let (local, remote) = upload;
        let compression = "-z";
        let receive_args: Vec<_> = [
            "/bin/sh",
            "-c",
            format!(r#"mkdir -p "$1" && exec tar -C "$1" {} -xv"#, compression)
                .as_str(),
            "-",
            remote,
        ]
        .into_iter()
        .map(String::from)
        .collect();

        // TODO: there must be a more elegant way to do this
        let kubectl_args = args
            .into_iter()
            .cloned()
            .map(|a| if a == "-it" { "-i".to_string() } else { a })
            .chain(receive_args);
        eprintln!("Uploading {} to {}", local, remote);
        let mut tar_cmd = Command::new("tar")
            .args(&["-C", local, "-c", compression, "."])
            .stdout(Stdio::piped())
            .spawn()?;
        let mut kubectl_cmd = kubectl
            .command(kubectl_args)
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
